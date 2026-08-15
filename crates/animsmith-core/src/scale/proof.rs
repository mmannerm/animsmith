//! Private scale proof, residual, sampling, and exact-discharge implementation.
//!
//! This module independently derives every numeric expectation checked against
//! a candidate. It deliberately does not import the reference writer's
//! expected values, connector products, or factor association.

use super::numeric::{
    column_operand_magnitude, mat4_abs, matrix_magnitude, matrix_residual,
    product_operand_magnitude, scale_translation_only,
};
use super::planning::{check_factor_narrows, validate_plan_document_inventory};
use super::reference::ScaleCandidate;
use super::validation::{
    WorldBonePose, WorldPose, affected_skin_instance_indices, child_translation_rounding_magnitude,
    instance_bind, local_rest_matrix, rest_world_pose, source_node_index_map, stored_instance_bind,
    validate_candidate_structure, validate_scale_input,
};
use super::{
    ProofResidualKind, ScaleError, ScaleFieldDisposition, ScaleFieldTarget, ScaleOperation,
    ScalePlan, ScaleProofObligation, ScaleRewriteRule, ScaleSourceNodeKind, ScaleSourceRestField,
    ScaleTolerancePolicy,
};
use crate::model::{
    BoneId, Clip, Document, DocumentShapeError, Interpolation, MeshInstanceShapeViolation,
    Primitive, Property, Skeleton, SourceNodeAsset, SourceNodeLocalRest, TrackValues, Transform,
    affine_axis_lengths, average_affine_axis_length, mat4_is_finite,
};
use crate::sample::{TrackSample, sample_track};
use glam::{DMat3, DMat4, DVec3, Mat3, Mat4, Quat, Vec3, Vec4};
use std::collections::{BTreeMap, BTreeSet};

impl ScalePlan {
    fn is_whole_document(&self) -> bool {
        matches!(self.compiled, super::ScaleCompiledPlan::WholeDocument(_))
    }

    fn has_obligation(&self, expected: ScaleProofObligation) -> bool {
        self.obligations().contains(&expected)
    }

    pub(super) fn rest_obligation(&self) -> Option<(&[BoneId], bool)> {
        if self.has_obligation(ScaleProofObligation::RestWorldAndUnitScale) {
            Some((self.affected_nodes(), true))
        } else if self.has_obligation(ScaleProofObligation::RestWorld) {
            Some((self.affected_nodes(), false))
        } else {
            None
        }
    }

    fn transform_only_nodes(&self) -> Option<&[BoneId]> {
        self.has_obligation(ScaleProofObligation::TransformOnlyAffine)
            .then(|| self.transform_only_attachments())
    }

    fn has_key_translations(&self) -> bool {
        self.has_obligation(ScaleProofObligation::KeyTranslations)
    }

    fn has_cubic_interiors(&self) -> bool {
        self.has_obligation(ScaleProofObligation::CubicInteriors)
    }

    fn trajectory_nodes(&self) -> Option<&[BoneId]> {
        self.has_obligation(ScaleProofObligation::Trajectories)
            .then(|| self.affected_nodes())
    }

    fn has_skin_and_bounds(&self) -> bool {
        self.has_obligation(ScaleProofObligation::SkinAndBounds)
    }

    fn has_unaffected_binds(&self) -> bool {
        self.has_obligation(ScaleProofObligation::UnaffectedInverseBinds)
    }
}

/// Independently compose one proof-side connector product.
///
/// This intentionally does not call the reference writer's connector-product
/// cache: the raw source-projection check must not certify a writer bug by
/// deriving its expectation through the writer's own product cache.
fn proof_connector_product(
    connector_tail: usize,
    by_source_index: &BTreeMap<usize, &SourceNodeAsset>,
    connector_product_by_tail: &mut BTreeMap<usize, DMat4>,
) -> Result<DMat4, ScaleError> {
    let mut suffix = Vec::new();
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
        let node = by_source_index
            .get(&cursor)
            .ok_or(ScaleError::IncompleteClosure {
                reason: "dangling_connector_source_node_index",
            })?;
        if node.bone.is_some() {
            break DMat4::IDENTITY;
        }
        suffix.push(cursor);
        cursor = node
            .parent_source_node_index
            .ok_or(ScaleError::IncompleteClosure {
                reason: "connector_without_projected_ancestor",
            })?;
    };
    while let Some(source) = suffix.pop() {
        let node = by_source_index
            .get(&source)
            .ok_or(ScaleError::IncompleteClosure {
                reason: "dangling_connector_source_node_index",
            })?;
        product *= local_rest_matrix(&node.local_rest).as_dmat4();
        connector_product_by_tail.insert(source, product);
    }
    connector_product_by_tail
        .get(&connector_tail)
        .copied()
        .ok_or(ScaleError::IncompleteClosure {
            reason: "empty_connector_bridge",
        })
}

/// Independently derive the exact f32 local that a bridged projected source
/// row must carry in the candidate.
fn proof_expected_bridged_source_local(
    local_rest: &SourceNodeLocalRest,
    connector: DMat4,
    s_parent: f32,
    s_node: f32,
    bone: BoneId,
) -> Result<SourceNodeLocalRest, ScaleError> {
    if s_parent == 1.0 && s_node == 1.0 {
        return Ok(local_rest.clone());
    }
    let inverse = DMat3::from_cols(
        connector.x_axis.truncate(),
        connector.y_axis.truncate(),
        connector.z_axis.truncate(),
    )
    .inverse();
    let offset = inverse * (connector.w_axis.truncate() * (f64::from(s_parent) - 1.0));
    if !inverse.x_axis.is_finite()
        || !inverse.y_axis.is_finite()
        || !inverse.z_axis.is_finite()
        || !offset.is_finite()
    {
        return Err(ScaleError::NonFiniteTransform { node: bone });
    }
    let expected = match local_rest {
        SourceNodeLocalRest::Trs {
            translation,
            rotation,
            scale,
        } => SourceNodeLocalRest::Trs {
            translation: (translation.as_dvec3() * f64::from(s_parent) + offset).as_vec3(),
            rotation: *rotation,
            scale: (scale.as_dvec3() * (f64::from(s_parent) / f64::from(s_node))).as_vec3(),
        },
        SourceNodeLocalRest::Matrix(matrix) => {
            let ratio = f64::from(s_parent) / f64::from(s_node);
            let rebase_linear_column = |column: Vec4| {
                (column.truncate().as_dvec3() * ratio)
                    .as_vec3()
                    .extend(column.w)
            };
            let translation =
                (matrix.w_axis.truncate().as_dvec3() * f64::from(s_parent) + offset).as_vec3();
            SourceNodeLocalRest::Matrix(Mat4::from_cols(
                rebase_linear_column(matrix.x_axis),
                rebase_linear_column(matrix.y_axis),
                rebase_linear_column(matrix.z_axis),
                translation.extend(matrix.w_axis.w),
            ))
        }
    };
    if !mat4_is_finite(local_rest_matrix(&expected)) {
        return Err(ScaleError::NonFiniteTransform { node: bone });
    }
    Ok(expected)
}

fn vec3_bits_equal(left: Vec3, right: Vec3) -> bool {
    left.to_array().map(f32::to_bits) == right.to_array().map(f32::to_bits)
}

fn quat_bits_equal(left: Quat, right: Quat) -> bool {
    left.to_array().map(f32::to_bits) == right.to_array().map(f32::to_bits)
}

fn source_rest_field_bits_equal(
    left: &SourceNodeLocalRest,
    right: &SourceNodeLocalRest,
    field: ScaleSourceRestField,
) -> bool {
    match (left, right, field) {
        (
            SourceNodeLocalRest::Trs {
                translation: left, ..
            },
            SourceNodeLocalRest::Trs {
                translation: right, ..
            },
            ScaleSourceRestField::Translation,
        )
        | (
            SourceNodeLocalRest::Trs { scale: left, .. },
            SourceNodeLocalRest::Trs { scale: right, .. },
            ScaleSourceRestField::Scale,
        ) => vec3_bits_equal(*left, *right),
        (
            SourceNodeLocalRest::Trs { rotation: left, .. },
            SourceNodeLocalRest::Trs {
                rotation: right, ..
            },
            ScaleSourceRestField::Rotation,
        ) => quat_bits_equal(*left, *right),
        (
            SourceNodeLocalRest::Matrix(left),
            SourceNodeLocalRest::Matrix(right),
            ScaleSourceRestField::MatrixLinear,
        ) => {
            vec3_bits_equal(left.x_axis.truncate(), right.x_axis.truncate())
                && vec3_bits_equal(left.y_axis.truncate(), right.y_axis.truncate())
                && vec3_bits_equal(left.z_axis.truncate(), right.z_axis.truncate())
        }
        (
            SourceNodeLocalRest::Matrix(left),
            SourceNodeLocalRest::Matrix(right),
            ScaleSourceRestField::MatrixTranslation,
        ) => vec3_bits_equal(left.w_axis.truncate(), right.w_axis.truncate()),
        (
            SourceNodeLocalRest::Matrix(left),
            SourceNodeLocalRest::Matrix(right),
            ScaleSourceRestField::MatrixHomogeneous,
        ) => {
            [left.x_axis.w, left.y_axis.w, left.z_axis.w, left.w_axis.w].map(f32::to_bits)
                == [
                    right.x_axis.w,
                    right.y_axis.w,
                    right.z_axis.w,
                    right.w_axis.w,
                ]
                .map(f32::to_bits)
        }
        _ => false,
    }
}

fn f32_values_within_scale_tolerance<const N: usize>(
    left: [f32; N],
    right: [f32; N],
    tolerance: &ScaleTolerancePolicy,
) -> bool {
    left.into_iter().zip(right).all(|(left, right)| {
        let left = f64::from(left);
        let right = f64::from(right);
        let residual = (left - right).abs();
        let limit = tolerance.scalar_tolerance(left, right);
        residual.is_finite() && limit.is_finite() && residual <= limit
    })
}

fn rewritten_source_rest_field_within_tolerance(
    expected: &SourceNodeLocalRest,
    actual: &SourceNodeLocalRest,
    field: ScaleSourceRestField,
    tolerance: &ScaleTolerancePolicy,
) -> bool {
    match (expected, actual, field) {
        (
            SourceNodeLocalRest::Trs {
                translation: expected,
                ..
            },
            SourceNodeLocalRest::Trs {
                translation: actual,
                ..
            },
            ScaleSourceRestField::Translation,
        )
        | (
            SourceNodeLocalRest::Trs {
                scale: expected, ..
            },
            SourceNodeLocalRest::Trs { scale: actual, .. },
            ScaleSourceRestField::Scale,
        ) => f32_values_within_scale_tolerance(expected.to_array(), actual.to_array(), tolerance),
        (
            SourceNodeLocalRest::Matrix(expected),
            SourceNodeLocalRest::Matrix(actual),
            ScaleSourceRestField::MatrixLinear,
        ) => f32_values_within_scale_tolerance(
            [
                expected.x_axis.x,
                expected.x_axis.y,
                expected.x_axis.z,
                expected.y_axis.x,
                expected.y_axis.y,
                expected.y_axis.z,
                expected.z_axis.x,
                expected.z_axis.y,
                expected.z_axis.z,
            ],
            [
                actual.x_axis.x,
                actual.x_axis.y,
                actual.x_axis.z,
                actual.y_axis.x,
                actual.y_axis.y,
                actual.y_axis.z,
                actual.z_axis.x,
                actual.z_axis.y,
                actual.z_axis.z,
            ],
            tolerance,
        ),
        (
            SourceNodeLocalRest::Matrix(expected),
            SourceNodeLocalRest::Matrix(actual),
            ScaleSourceRestField::MatrixTranslation,
        ) => f32_values_within_scale_tolerance(
            expected.w_axis.truncate().to_array(),
            actual.w_axis.truncate().to_array(),
            tolerance,
        ),
        // Rewrites never own rotation or the matrix homogeneous row. Keep
        // these impossible combinations fail-closed instead of silently
        // assigning them a numeric policy.
        _ => false,
    }
}

/// Independently derive the raw authored local expected by one rewrite row.
///
/// This is deliberately proof-owned arithmetic. In particular, direct rows
/// spell out their `f32` association here instead of calling the builder's
/// rebase helpers, while connector rows use the separately implemented
/// proof-side connector product and widened affine derivation.
fn proof_expected_rewritten_source_local(
    source: &Document,
    plan: &ScalePlan,
    affected: &BTreeSet<BoneId>,
    source_node: &SourceNodeAsset,
    rule: ScaleRewriteRule,
    connector_products: &mut BTreeMap<usize, DMat4>,
    source_nodes: &BTreeMap<usize, &SourceNodeAsset>,
) -> Result<SourceNodeLocalRest, ScaleError> {
    match rule {
        ScaleRewriteRule::WholeDocumentLength => {
            let q = check_factor_narrows(plan.common_factor(), plan.common_factor())?;
            Ok(match &source_node.local_rest {
                SourceNodeLocalRest::Trs {
                    translation,
                    rotation,
                    scale,
                } => SourceNodeLocalRest::Trs {
                    translation: Vec3::new(translation.x * q, translation.y * q, translation.z * q),
                    rotation: *rotation,
                    scale: *scale,
                },
                SourceNodeLocalRest::Matrix(matrix) => {
                    SourceNodeLocalRest::Matrix(Mat4::from_cols(
                        matrix.x_axis,
                        matrix.y_axis,
                        matrix.z_axis,
                        Vec4::new(
                            matrix.w_axis.x * q,
                            matrix.w_axis.y * q,
                            matrix.w_axis.z * q,
                            matrix.w_axis.w,
                        ),
                    ))
                }
            })
        }
        ScaleRewriteRule::RestBindSourceLocal { connector_tail } => {
            let bone = source_node
                .bone
                .ok_or(ScaleError::SourceNodeNotNormalized {
                    source_node_index: source_node.source_node_index,
                })?;
            let parent = source
                .skeleton
                .bones
                .get(bone)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: bone })?
                .parent;
            let s = check_factor_narrows(plan.common_factor(), plan.common_factor())?;
            let s_parent = if parent.is_some_and(|parent| affected.contains(&parent)) {
                s
            } else {
                1.0
            };
            let s_node = if affected.contains(&bone) { s } else { 1.0 };
            if let Some(connector_tail) = connector_tail {
                let connector =
                    proof_connector_product(connector_tail, source_nodes, connector_products)?;
                return proof_expected_bridged_source_local(
                    &source_node.local_rest,
                    connector,
                    s_parent,
                    s_node,
                    bone,
                );
            }
            Ok(match &source_node.local_rest {
                SourceNodeLocalRest::Trs {
                    translation,
                    rotation,
                    scale,
                } => SourceNodeLocalRest::Trs {
                    translation: Vec3::new(
                        translation.x * s_parent,
                        translation.y * s_parent,
                        translation.z * s_parent,
                    ),
                    rotation: *rotation,
                    scale: Vec3::new(
                        scale.x * (s_parent / s_node),
                        scale.y * (s_parent / s_node),
                        scale.z * (s_parent / s_node),
                    ),
                },
                SourceNodeLocalRest::Matrix(matrix) => {
                    let inverse_node = 1.0 / s_node;
                    let linear = |column: Vec4| {
                        Vec4::new(
                            column.x * s_parent * inverse_node,
                            column.y * s_parent * inverse_node,
                            column.z * s_parent * inverse_node,
                            column.w * inverse_node,
                        )
                    };
                    SourceNodeLocalRest::Matrix(Mat4::from_cols(
                        linear(matrix.x_axis),
                        linear(matrix.y_axis),
                        linear(matrix.z_axis),
                        Vec4::new(
                            matrix.w_axis.x * s_parent,
                            matrix.w_axis.y * s_parent,
                            matrix.w_axis.z * s_parent,
                            matrix.w_axis.w,
                        ),
                    ))
                }
            })
        }
        ScaleRewriteRule::RestBindParentBasis
        | ScaleRewriteRule::RestBindLocalScale
        | ScaleRewriteRule::RestBindNodeBasis => Err(ScaleError::PlanDocumentMismatch {
            reason: "invalid_source_local_rewrite_rule",
        }),
    }
}

/// Discharge rewritten authored source-local rows against a proof-owned
/// analytic expectation. Direct raw format adapters may narrow an authored
/// value before or after applying the same factor, so those rows use the
/// published scale tolerance. Connector-bridged successors remain bit-exact:
/// that core-only path has no second frontend narrowing boundary.
fn check_rewritten_source_field_dispositions(
    source: &Document,
    candidate: &Document,
    plan: &ScalePlan,
    affected: &BTreeSet<BoneId>,
    tolerance: &ScaleTolerancePolicy,
    discharged: &mut BTreeSet<usize>,
) -> Result<(), ScaleError> {
    let source_nodes = source_node_index_map(source);
    let candidate_nodes = source_node_index_map(candidate);
    let mut connector_products = BTreeMap::new();
    for (row_index, row) in plan.field_rows().iter().enumerate() {
        let (
            ScaleFieldTarget::SourceNodeRest {
                source_node_index,
                field,
            },
            ScaleFieldDisposition::Rewrite(rule),
        ) = (row.target, row.disposition)
        else {
            continue;
        };
        let before =
            source_nodes
                .get(&source_node_index)
                .ok_or(ScaleError::CandidateStructureMismatch {
                    reason: "rewritten_source_node_missing",
                })?;
        let after = candidate_nodes.get(&source_node_index).ok_or(
            ScaleError::CandidateStructureMismatch {
                reason: "rewritten_source_node_missing",
            },
        )?;
        let expected = proof_expected_rewritten_source_local(
            source,
            plan,
            affected,
            before,
            rule,
            &mut connector_products,
            &source_nodes,
        )?;
        let bridged = matches!(
            rule,
            ScaleRewriteRule::RestBindSourceLocal {
                connector_tail: Some(_)
            }
        );
        let matches = if bridged {
            source_rest_field_bits_equal(&expected, &after.local_rest, field)
        } else {
            rewritten_source_rest_field_within_tolerance(
                &expected,
                &after.local_rest,
                field,
                tolerance,
            )
        };
        if !matches {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: if bridged {
                    "bridged_source_local_mismatch"
                } else {
                    "field_disposition_mismatch"
                },
            });
        }
        mark_field_row_discharged(discharged, row_index)?;
    }
    Ok(())
}

/// Discharge preserve-exact authored source-local rows after semantic
/// residuals succeed. These are raw fields both core builders copy directly;
/// normalized fields may be independently re-derived by a format frontend
/// and remain governed by the existing versioned residual policy.
fn check_preserved_field_dispositions(
    source: &Document,
    candidate: &Document,
    plan: &ScalePlan,
    discharged: &mut BTreeSet<usize>,
) -> Result<(), ScaleError> {
    let source_nodes = source_node_index_map(source);
    let candidate_nodes = source_node_index_map(candidate);
    let mut connector_sources = BTreeSet::new();
    let mut bridged_successors = BTreeSet::new();
    for row in plan.ledger().source_topology() {
        match row.kind() {
            ScaleSourceNodeKind::Connector => {
                connector_sources.insert(row.source_node_index());
            }
            ScaleSourceNodeKind::Projected {
                incoming_connector_tail: Some(_),
                ..
            } => {
                bridged_successors.insert(row.source_node_index());
            }
            ScaleSourceNodeKind::Projected { .. } | ScaleSourceNodeKind::OutsideDomain { .. } => {}
        }
    }
    for (row_index, row) in plan.field_rows().iter().enumerate() {
        let (
            ScaleFieldTarget::SourceNodeRest {
                source_node_index,
                field,
            },
            ScaleFieldDisposition::PreserveExact,
        ) = (row.target, row.disposition)
        else {
            continue;
        };
        let exact = match (
            source_nodes.get(&source_node_index),
            candidate_nodes.get(&source_node_index),
        ) {
            (Some(before), Some(after)) => {
                source_rest_field_bits_equal(&before.local_rest, &after.local_rest, field)
            }
            // Unavailable coverage carries no authoritative raw-row identity;
            // the compiler emits no source field rows for it.
            (None, None) => true,
            _ => false,
        };
        if !exact {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: if connector_sources.contains(&source_node_index) {
                    "connector_source_local_mismatch"
                } else if bridged_successors.contains(&source_node_index) {
                    "bridged_source_local_mismatch"
                } else {
                    "field_disposition_mismatch"
                },
            });
        }
        mark_field_row_discharged(discharged, row_index)?;
    }
    Ok(())
}

fn mark_field_row_discharged(
    discharged: &mut BTreeSet<usize>,
    row_index: usize,
) -> Result<(), ScaleError> {
    if !discharged.insert(row_index) {
        return Err(ScaleError::PlanDocumentMismatch {
            reason: "field_row_discharged_twice",
        });
    }
    Ok(())
}

fn finish_field_row_discharge(
    plan: &ScalePlan,
    discharged: &BTreeSet<usize>,
) -> Result<(), ScaleError> {
    let expected: BTreeSet<_> = (0..plan.field_rows().len()).collect();
    if *discharged != expected {
        return Err(ScaleError::PlanDocumentMismatch {
            reason: "field_row_not_discharged",
        });
    }
    Ok(())
}

// --- Proof -------------------------------------------------------------

mod residual {
    /// One proof claim's maximum residual and the comparisons behind it.
    ///
    /// A maximum of `0.0` can mean either an exact measurement or that no
    /// comparison was made. [`Self::evaluated`] distinguishes those cases.
    /// The two measurements are intentionally one read-only value so a
    /// consumer cannot pair one claim's maximum with another claim's count.
    #[derive(Debug, Clone, Copy, PartialEq)]
    #[non_exhaustive]
    pub struct ScaleProofResidual {
        max: f64,
        comparisons: usize,
    }

    impl ScaleProofResidual {
        /// The maximum residual observed across this claim's comparisons.
        #[must_use]
        pub fn max(self) -> f64 {
            self.max
        }

        /// The number of comparisons behind [`Self::max`].
        #[must_use]
        pub fn comparisons(self) -> usize {
            self.comparisons
        }

        /// Whether the proof evaluated this claim at least once.
        #[must_use]
        pub fn evaluated(self) -> bool {
            self.comparisons != 0
        }

        pub(super) const EMPTY: Self = Self {
            max: 0.0,
            comparisons: 0,
        };

        pub(super) fn record(&mut self, observed: f64) {
            self.max = self.max.max(observed);
            self.comparisons += 1;
        }
    }

    #[cfg(doctest)]
    mod api_contract {
        /// Compile-fail coverage for the removed split API. Each field stays in
        /// its own compilation unit so restoring one cannot be masked by a
        /// different missing field.
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.rest_translation_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.rest_translation_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.rest_rotation_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.rest_rotation_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.unit_scale_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.unit_scale_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.transform_only_affine_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.transform_only_affine_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.track_value_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.track_value_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.mesh_position_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.mesh_position_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.key_translation_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.key_translation_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.cubic_interior_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.cubic_interior_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.trajectory_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.trajectory_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.skin_matrix_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.skin_matrix_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.bounds_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.bounds_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.unaffected_inverse_bind_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.unaffected_inverse_bind_comparisons;
        /// }
        /// ```
        ///
        /// Direct construction and member-by-member mutation are also unavailable:
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProofResidual;
        ///
        /// let _ = ScaleProofResidual {
        ///     max: 0.0,
        ///     comparisons: 1,
        /// };
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProofResidual;
        ///
        /// fn replace_max(mut residual: ScaleProofResidual) {
        ///     residual.max = 1.0;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProofResidual;
        ///
        /// fn replace_count(mut residual: ScaleProofResidual) {
        ///     residual.comparisons = 1;
        /// }
        /// ```

        struct RemovedSplitFields;
    }
}

pub use residual::ScaleProofResidual;

/// Observed residual maxima from [`prove_scale`], reported against
/// [`ScalePlan::tolerance_policy`], each paired with the number of
/// comparisons that produced it.
///
/// **Read the paired value, not just its maximum.** A maximum alone cannot
/// distinguish "compared, no deviation" from "nothing to compare": both read
/// `0.0`, because every maximum starts there and is raised only by a loop
/// that may have zero iterations. So a field for an obligation the plan does
/// not require (see [`ScaleProofObligation`]) reports a
/// [`ScaleProofResidual`] whose maximum and count are both zero. A `0.0`
/// maximum with a count above zero is a measurement; a zero count is an
/// absence, which DESIGN.md Appendix D §D.6 requires an evidence record to
/// publish as an absence rather than as a checked zero.
///
/// The counts are measurements, not proxies derived from obligation presence:
/// each is stored with its maximum by the single private recording method
/// every comparison in [`prove_scale`] funnels through. The pair's fields are
/// private, so no producer can combine one claim's count with another claim's
/// maximum.
/// No comparison can raise a residual without being counted, and
/// no count can rise without a comparison having been checked against the
/// tolerance policy. A [`ScaleProofObligation`] may declare a proof walk, or
/// name a row-driven claim discharged through the canonical field inventory;
/// neither role substitutes for the comparison count recorded here.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ScaleProof {
    /// The tolerance policy every residual below was checked against.
    pub tolerance_policy: ScaleTolerancePolicy,
    /// Maximum rest-world translation residual, with one comparison per
    /// affected node.
    pub rest_translation: ScaleProofResidual,
    /// Maximum rest-world rotation residual, in radians, directly comparable
    /// to [`ScaleTolerancePolicy::rotation_residual_radians`].
    ///
    /// Measured as a double-cover-aware quaternion chord length
    /// `|q1 - q2| = 2 * sin(theta / 4)` and reported as the angle
    /// `theta = 4 * asin(chord / 2)` that chord represents, so this value and
    /// the tolerance it is checked against carry the same unit.
    /// Its count is one per affected node.
    pub rest_rotation: ScaleProofResidual,
    /// Maximum postcondition unit-scale residual, with one comparison per
    /// affected node when the rest/bind plan declares the postcondition.
    pub unit_scale: ScaleProofResidual,
    /// Maximum transform-only attachment full-affine residual (rest/bind
    /// only), with one probe-point comparison per attachment.
    pub transform_only_affine: ScaleProofResidual,
    /// Maximum per-element animation-track value residual, across rewritten
    /// translation elements and every retained rotation/scale element. Its
    /// count is one per track element across every clip.
    pub track_value: ScaleProofResidual,
    /// Maximum per-vertex base mesh `POSITION` residual, with one comparison
    /// per vertex.
    pub mesh_position: ScaleProofResidual,
    /// Maximum residual at any affected-track keyframe time, with one
    /// comparison per affected translation track per key time.
    pub key_translation: ScaleProofResidual,
    /// Maximum residual at any bounded cubic-segment interior time, with one
    /// comparison per affected translation track per interior time.
    pub cubic_interior: ScaleProofResidual,
    /// Maximum sampled world-space trajectory residual, with one comparison
    /// per affected node per sample time.
    pub trajectory: ScaleProofResidual,
    /// Maximum skin-matrix (`W * B`) component residual, across rest and
    /// every sampled key/cubic-interior time. Its count is one per skin slot
    /// of every affected skinned instance at each evaluated pose.
    pub skin_matrix: ScaleProofResidual,
    /// Maximum skinned mesh bounds residual, across rest and every sampled
    /// key/cubic-interior time. Its count is six per evaluated pose.
    pub bounds: ScaleProofResidual,
    /// Maximum stored inverse-bind residual over every skin slot outside the
    /// affected closure (see [`ProofResidualKind::UnaffectedInverseBind`]).
    /// Its count is one per stored slot compared outside the closure and zero
    /// unless the plan declares
    /// [`ScaleProofObligation::UnaffectedInverseBinds`].
    pub unaffected_inverse_bind: ScaleProofResidual,
    /// The operation's observed factor, re-derived by this proof from the
    /// documents it was handed rather than copied from
    /// [`ScalePlan::observed_factor`], so evidence does not depend on
    /// planning having recorded it.
    ///
    /// **Reported, not checked.** This is a measurement, not an obligation:
    /// nothing here compares it against [`ScalePlan::common_factor`]. The
    /// declared/observed agreement is the *input* contract §D.1 states, and
    /// planning already enforces it ([`ScaleError::FactorMismatch`]); what
    /// binds the candidate is the postcondition
    /// ([`ProofResidualKind::UnitScale`]) that §D.1 derives from that band.
    ///
    /// Measured from `source`, not from `candidate`. That is a choice, not an
    /// impossibility: a rest/bind candidate does record what its source
    /// measured. `build_rest_bind` rebases an affected node's local scale by
    /// `s_parent / s_node` with `s_node` the *declared* factor, and the
    /// affected closure never contains the scaled root's parent, so `s_parent`
    /// is one there and the candidate's composed root scale is exactly
    /// `s_observed / s_declared` — unit only when the two agree, which the
    /// input band admits without requiring. The candidate route is real and it
    /// is accurate: `plan.common_factor() * average_affine_axis_length(
    /// affine_axis_lengths(candidate root world linear))` recovers the
    /// measurement to a relative error below `2^-24`, the half-ulp of the one
    /// binary32 rounding that round trip costs at unit magnitude. Swept over
    /// all 215 binary32 values
    /// the `1e-5` band admits around a declared `0.01`, the worst is
    /// `5.9604613e-8`; over this module's own `0.01`-factor fixtures it is
    /// `4.84e-8`, at a source root of `0.010_000_099`.
    ///
    /// It is not the route taken, for four reasons:
    ///
    /// - **Independence.** [`prove_scale`] does not require `candidate` to
    ///   have come from reference construction; checking one that did not
    ///   is the reason it exists. Reading the reported measurement off the
    ///   artifact under test would let that artifact pick its own evidence
    ///   value. The observed factor is a fact about the *input*, so the input
    ///   is where it is measured.
    /// - **Precision.** The candidate route divides by the declared factor in
    ///   `f32` and multiplies back in `f64`, so it reports a rounded
    ///   neighbour of the source measurement rather than the measurement.
    /// - **Sign.** The nearest already-published proxy,
    ///   [`Self::unit_scale`]'s `max()`, is an absolute value, so
    ///   `common_factor * (1 + unit_scale.max())` reconstructs
    ///   `s_observed` only when the observed factor is the *larger* of the
    ///   two and reflects it about the declared factor when it is not.
    /// - **Attribution.** That residual is also a maximum over every affected
    ///   node rather than a value read at the scaled root, so it does not
    ///   name the node §D.6 defines the observed factor at.
    ///
    /// See [`ScalePlan::observed_factor`] for how each operation defines it;
    /// for a whole-document conversion this is the declared factor, there
    /// being nothing to measure — and there the candidate route genuinely
    /// does not exist, because `build_whole_document` rewrites translations
    /// only and leaves every composed scale exactly as it found it.
    pub observed_factor: f64,
    /// The observed factor [`super::plan_scale`] measured, copied from
    /// [`ScalePlan::observed_factor`].
    ///
    /// The record carries both witnesses because they are measured from
    /// genuinely different state and neither is derivable from the other:
    /// this one from the raw source projection (`SourceNodeAsset::local_rest`
    /// composed through `parent_source_node_index`),
    /// [`Self::observed_factor`] from the normalized skeleton (`Bone::rest`
    /// composed through `world_rest_matrices`). That independence is the
    /// property DESIGN.md Appendix D §D.6 wants of a second witness, and it
    /// is why the two are generally not equal. Carrying only one of them
    /// would leave a reader unable to tell which was reported; carrying both
    /// with no stated relationship would leave them unable to tell which to
    /// trust, which is what [`Self::observed_factor_divergence`] answers.
    ///
    /// For [`ScaleOperation::WholeDocumentLinearUnits`] both are the declared
    /// factor, there being nothing to measure, and the divergence is exactly
    /// zero.
    pub planned_observed_factor: f64,
    /// How far apart the two observed factors are:
    /// `abs(planned - proved) / max(abs(planned), abs(proved))`.
    ///
    /// Recorded explicitly rather than left for a consumer to compute, so the
    /// evidence record states the relationship between its own two witnesses
    /// instead of presenting two numbers that both answer to "the observed
    /// factor". Compare it against
    /// [`ScaleTolerancePolicy::observed_factor_divergence_ceiling`], which is
    /// how far apart the design expects them to be and why — a consumer does
    /// not have to re-derive that ceiling by summing two separate policy
    /// fields.
    ///
    /// **Reported, not checked.** Nothing refuses a document for exceeding
    /// the ceiling; see that method for what the ceiling does and does not
    /// guarantee. The two *chains* the witnesses compose through are already
    /// required to agree: under
    /// [`crate::model::SourceSkeletonCoverage::Complete`] coverage a document
    /// whose projection and skeleton describe different trees is refused
    /// before either witness is taken. What nothing reconciles is the two
    /// *readings* — [`crate::model::SourceNodeAsset::local_rest`] and
    /// [`crate::model::Bone::rest`] stay separately stored and separately
    /// composed, which is why both witnesses exist at all. A divergence
    /// beyond the ceiling is therefore a fact about how far apart the input's
    /// two stored descriptions of one rest pose are, worth surfacing, not a
    /// residual this proof owns.
    pub observed_factor_divergence: f64,
    /// Number of distinct times sampled across all clips.
    pub sample_time_count: usize,
    // Calibration-only raw f32-rounding demand maxima. These are intentionally
    // private and test-only: evidence publishes residuals and comparison
    // counts, while the ignored calibration needs the per-comparison
    // `observed / (base * ulp)` maximum that the proof actually checked.
    // Keeping it on the test build's proof makes calibration consume the
    // production comparison instead of recomputing poses, slots, bounds, or
    // bases, without changing the release proof layout or runtime work.
    #[cfg(test)]
    pub(super) rest_translation_f32_rounding_demand: f64,
    #[cfg(test)]
    pub(super) trajectory_f32_rounding_demand: f64,
    #[cfg(test)]
    pub(super) skin_matrix_f32_rounding_demand: f64,
    #[cfg(test)]
    pub(super) bounds_f32_rounding_demand: f64,
    #[cfg(test)]
    pub(super) unaffected_inverse_bind_f32_rounding_demand: f64,
}

/// Independently re-derive and check every claim [`ScalePlan`] makes.
///
/// Proof runs on the in-memory candidate, re-deriving world matrices,
/// sampled trajectories, skin matrices, and bounds from `source` and
/// `candidate` rather than trusting how they were built. Numerical residuals
/// use [`ScaleTolerancePolicy::scalar_tolerance`] computed from that
/// comparison's own actual before/after magnitudes, never a proxy such as the
/// plan's declared factor. Discrete topology and the complete rest-world
/// affine outside a rest/bind closure are exact unchanged-domain invariants.
/// The world comparison is a semantic placement claim; exact local write-set
/// parity is a separate artifact/ledger obligation. Neither `source` nor
/// `candidate` need be numerically identical to the document `plan` was
/// computed against, but re-deriving `source`'s structural planning inventory
/// must produce the same affected domain and proof obligations.
///
/// # Errors
///
/// Returns [`ScaleError::PlanDocumentMismatch`] when the supplied source
/// derives a different proof inventory, any planning/selector error surfaced
/// while re-deriving that inventory, [`ScaleError::CandidateStructureMismatch`]
/// when an exact source/candidate invariant differs,
/// [`ScaleError::ProofResidualExceeded`] for the first residual that exceeds
/// [`ScalePlan::tolerance_policy`], or [`ScaleError::MissingProofEvidence`] if
/// an obligation the plan declares provable has no counterpart evidence in
/// `candidate`.
///
/// Two of the claims checked here are not gated by
/// [`ScaleProofObligation`]: the per-element animation-track values
/// ([`ProofResidualKind::TrackValue`]), base mesh `POSITION`
/// ([`ProofResidualKind::MeshPosition`]). They compare every element of
/// every track and every base mesh against that element's own domain's
/// analytic expectation — the declared multiplier where the plan rewrites
/// that domain, the retained value where it does not — and so are owed by
/// every plan. Base `POSITION` belongs with the track-value comparison, not
/// with the unaffected binds, which is worth naming because it is easy to get
/// backwards: a whole-document plan *does* rewrite it
/// ([`ScaleRewriteRule::WholeDocumentLength`]), so its comparison is a
/// rewritten-value check, and it is unconditional because skinned bounds
/// would otherwise be its only witness — and they report a zero residual for
/// a document carrying no skinned instance at all. Neither admits
/// an obligation flag as a proxy for having run — see [`ScaleProof`], whose
/// comparison counts report what each of them actually walked.
///
/// [`ScaleProof::observed_factor`] is re-derived here from `source` rather
/// than copied from [`ScalePlan::observed_factor`]; it is reported as
/// evidence and is not itself an obligation. Both witnesses and the
/// divergence between them are recorded
/// ([`ScaleProof::planned_observed_factor`],
/// [`ScaleProof::observed_factor_divergence`]); none of the three is checked
/// against a band here.
pub fn prove_scale(
    source: &Document,
    candidate: &ScaleCandidate,
    plan: &ScalePlan,
) -> Result<ScaleProof, ScaleError> {
    let candidate = candidate.document();
    validate_plan_document_inventory(source, plan)?;
    validate_scale_input(candidate)?;
    validate_candidate_structure(source, candidate)?;
    let mut discharged_field_rows = BTreeSet::new();
    let tol = plan.tolerance_policy;
    let affected = plan.affected_set();
    let affected_skin_instances = if plan.has_skin_and_bounds() {
        affected_skin_instance_indices(source, &affected)
    } else {
        Vec::new()
    };
    let source_worlds = rest_world_pose(&source.skeleton)?;
    let candidate_worlds = rest_world_pose(&candidate.skeleton)?;

    // Rest/bind rewrites a strict hierarchy domain while promising that every
    // bone outside it keeps the same world rest. This is exact, not a new
    // tolerance: unchanged placement is the operation's semantic invariant,
    // and a relative tolerance would only admit larger and larger displacement
    // as authored coordinates grow. Compare the complete affine so an
    // in-place rotation or scale mutation cannot hide behind an unchanged
    // origin. Exact local-field/write-set parity is intentionally not inferred
    // from equal matrices; that belongs to the explicit artifact/ledger layer.
    //
    // Whole-document conversion has no complement, so the loop is naturally
    // empty there; topology parity still applies because that operation does
    // not rewrite parents either.
    if plan.has_obligation(ScaleProofObligation::ExactUnchangedWorldRest) {
        for node in (0..source.skeleton.bones.len()).filter(|node| !affected.contains(node)) {
            let before = source_worlds.bone(node)?.matrix;
            let after = candidate_worlds.bone(node)?.matrix;
            if before != after {
                return Err(ScaleError::CandidateStructureMismatch {
                    reason: "unaffected_world_rest_mismatch",
                });
            }
        }
    }
    let observed_factor = observed_factor_from_source(source, &source_worlds, plan)?;
    let mut proof = ScaleProof {
        tolerance_policy: tol,
        rest_translation: ScaleProofResidual::EMPTY,
        rest_rotation: ScaleProofResidual::EMPTY,
        unit_scale: ScaleProofResidual::EMPTY,
        transform_only_affine: ScaleProofResidual::EMPTY,
        track_value: ScaleProofResidual::EMPTY,
        mesh_position: ScaleProofResidual::EMPTY,
        key_translation: ScaleProofResidual::EMPTY,
        cubic_interior: ScaleProofResidual::EMPTY,
        trajectory: ScaleProofResidual::EMPTY,
        skin_matrix: ScaleProofResidual::EMPTY,
        bounds: ScaleProofResidual::EMPTY,
        unaffected_inverse_bind: ScaleProofResidual::EMPTY,
        observed_factor,
        planned_observed_factor: plan.observed_factor,
        observed_factor_divergence: relative_divergence(plan.observed_factor, observed_factor),
        sample_time_count: 0,
        #[cfg(test)]
        rest_translation_f32_rounding_demand: 0.0,
        #[cfg(test)]
        trajectory_f32_rounding_demand: 0.0,
        #[cfg(test)]
        skin_matrix_f32_rounding_demand: 0.0,
        #[cfg(test)]
        bounds_f32_rounding_demand: 0.0,
        #[cfg(test)]
        unaffected_inverse_bind_f32_rounding_demand: 0.0,
    };

    check_candidate_values(
        source,
        candidate,
        &affected,
        plan,
        &tol,
        &mut proof,
        &mut discharged_field_rows,
    )?;

    if let Some((rest_nodes, prove_unit_scale)) = plan.rest_obligation() {
        for &node in rest_nodes {
            let before = source_worlds.bone(node)?.matrix;
            let after_pose = candidate_worlds.bone(node)?;
            let after = after_pose.matrix;
            let after_chain = after_pose.translation_rounding_magnitude;
            let (translation_residual, before_mag, after_mag) = rest_node_residual(
                before,
                after,
                plan.is_whole_document(),
                plan.common_factor(),
            );
            // The magnitude is the parent chain's, not the surviving
            // translation's: a joint whose local offset points back along its
            // parent's world translation leaves a world translation the
            // difference of two much larger terms, carrying their rounding
            // error into a comparison whose own operands are small.
            //
            // The *candidate's* chain, not the source's and not the `max` of
            // the two. The residual is measured against the candidate's
            // arithmetic: whole-document conversion scales every translation
            // by the factor and leaves every linear part alone, so the two
            // chains are that factor apart — subject to the candidate's `f32`
            // narrowing of the factor, since the build scales by
            // `factor as f32` while this proof rebases by the `f64` factor, a
            // relative difference of at most `2^-24` (`1.49e-8` at `q = 0.1`,
            // whose `f32` is `0.10000000149011612`) that the rounding term
            // covers many times over — and the source's rounding is rebased by
            // the same factor before it is compared. The
            // residual therefore scales with `after_chain` at either end of
            // the factor range, and under rest/bind the two chains are equal
            // outright. `before_chain` can only ever over-provide — and under
            // a *shrinking* conversion it over-provides by `1/factor`, freezing
            // the band at the source rig's size while the candidate the band
            // is spent on gets smaller without limit.
            //
            // `a_cancelling_chain_under_conversion_holds_rest_translation_to_the_candidate_side`
            // pins that reading the *source* side alone refuses a correct
            // candidate under a growing conversion;
            // `a_shrinking_conversion_holds_rest_translation_to_the_candidate_s_own_chain`
            // pins the opposite direction, where the source side is the larger
            // one and reading it admits a `100x` larger build error; and
            // `the_rest_translation_v6_floor_is_an_adjacent_f32_transition`
            // pins the size of the term from above.
            check_and_track_f32_rounded(
                ProofResidualKind::RestTranslation,
                translation_residual,
                before_mag,
                after_mag,
                after_chain,
                &tol,
                &mut proof,
            )?;
            // Both operations leave every node's *local* rotation field
            // byte-identical (translation and scale are the only rewritten
            // channels), so proving world orientation preservation by
            // comparing local rotations directly avoids a lossy matrix
            // decomposition and, by composition, implies preserved world
            // orientation for every node in the chain.
            //
            // This is therefore an *equality* test on a field no build path
            // writes, not an angle measurement — and it must not be spelled
            // as one. `Quat::angle_between` does not normalize its operands,
            // so an authored quaternion with `|q| = 1 - eps` (routine in
            // glTF, and which `invariant-9` forbids the loader from
            // renormalizing) reports roughly `2 * sqrt(4 * eps)` against
            // itself: the perfectly ordinary key `[0, 0.7071067, 0,
            // 0.7071067]` measures `1.2e-3` against an identical copy, `120x`
            // the `1e-5` tolerance, and a correct candidate for a real rig is
            // rejected as `RestRotation`.
            let source_rotation = source
                .skeleton
                .bones
                .get(node)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?
                .rest
                .rotation;
            let candidate_rotation = candidate
                .skeleton
                .bones
                .get(node)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?
                .rest
                .rotation;
            // [`quat_equality_residual`] answers in *chord* space, and this
            // obligation's declared bound is an angle, so the chord is
            // converted to the angle it represents before it is either
            // reported or compared (see [`quat_residual_radians`]).
            let rotation_residual =
                quat_residual_radians(quat_equality_residual(source_rotation, candidate_rotation));
            record_and_check(
                ProofResidualKind::RestRotation,
                rotation_residual,
                tol.rotation_residual_radians,
                &mut proof,
            )?;
            if prove_unit_scale {
                let (after_scale, ..) = after.to_scale_rotation_translation();
                // Per-axis (L-infinity), per
                // [`ScaleTolerancePolicy::postcondition_unit_scale_residual`]:
                // "unit composed scale for every affected node" (DESIGN.md
                // Appendix D §D.6) is a per-axis claim, and measuring it
                // per-axis is what makes it commensurable with the scalar
                // relative common-factor band that gates the input. An L2
                // norm over the three axes reports `sqrt(3)` times the same
                // defect and therefore rejected candidates the very same
                // policy's input band had just accepted.
                let residual = (after_scale.x as f64 - 1.0)
                    .abs()
                    .max((after_scale.y as f64 - 1.0).abs())
                    .max((after_scale.z as f64 - 1.0).abs());
                record_and_check(
                    ProofResidualKind::UnitScale,
                    residual,
                    tol.postcondition_unit_scale_residual,
                    &mut proof,
                )?;
            }
        }
    }
    // The rest-world handler above is the semantic owner of all normalized
    // rest containers. Translation/rotation have direct residuals; scale is
    // intentionally owned at composed-world level (and, for rest/bind, by
    // the unit-scale postcondition) rather than by a new local-value band.
    for (row_index, row) in plan.field_rows().iter().enumerate() {
        if matches!(row.target, ScaleFieldTarget::BoneRest { .. }) {
            mark_field_row_discharged(&mut discharged_field_rows, row_index)?;
        }
    }

    if let Some(transform_only_nodes) = plan.transform_only_nodes() {
        // scale(1/s): the analytically expected basis correction `C_i` for
        // every node inside the affected domain (DESIGN.md Appendix D §D.2).
        let correction = Mat4::from_scale(Vec3::splat((1.0 / plan.common_factor()) as f32));
        // A fixed off-origin local probe point: transforming it through the
        // complete expected/actual world affine — rather than decomposing
        // to translation/rotation and checking only those — is what makes a
        // no-op (or any build that drops the linear-scale channel) provably
        // fail this check.
        let probe = Vec3::ONE;
        for &node in transform_only_nodes {
            let before = source_worlds.bone(node)?.matrix;
            let after = candidate_worlds.bone(node)?.matrix;
            let expected_point = (before * correction).transform_point3(probe).as_dvec3();
            let actual_point = after.transform_point3(probe).as_dvec3();
            let residual = (actual_point - expected_point).length();
            check_and_track(
                ProofResidualKind::TransformOnlyAffine,
                residual,
                expected_point.length(),
                actual_point.length(),
                &tol,
                &mut proof,
            )?;
        }
    }

    check_skin_and_bounds(
        source,
        candidate,
        &source_worlds,
        &candidate_worlds,
        &affected_skin_instances,
        plan,
        &tol,
        &mut proof,
    )?;
    if plan.has_unaffected_binds() {
        check_unaffected_instance_binds(source, candidate, &affected, &tol, &mut proof)?;
    }
    // The shared skin walk and unaffected-bind walk are the existing numeric
    // owners for stored slot binds. Bone convenience binds that are
    // unreferenced or shadowed, plus normals that neither scale operation
    // writes, remain ownership-only rows: changing their numeric policy here
    // would alter the accepted set.
    for (row_index, row) in plan.field_rows().iter().enumerate() {
        if matches!(
            row.target,
            ScaleFieldTarget::BoneInverseBind { .. }
                | ScaleFieldTarget::InstanceInverseBind { .. }
                | ScaleFieldTarget::MeshNormals { .. }
        ) {
            mark_field_row_discharged(&mut discharged_field_rows, row_index)?;
        }
    }

    let any_sampled_obligation = plan.has_key_translations()
        || plan.has_cubic_interiors()
        || plan.trajectory_nodes().is_some()
        || plan.has_skin_and_bounds();
    if any_sampled_obligation {
        // Harvested once, up front, for two reasons: the budget below has to
        // know the total sample count *before* the first sample is evaluated,
        // and re-harvesting per clip inside the loop would sort and dedup the
        // same key times twice.
        let mut clip_times = Vec::with_capacity(source.clips.len());
        let mut sample_times: u64 = 0;
        for clip in &source.clips {
            let times = clip_sample_times(clip, &affected);
            sample_times = sample_times
                .saturating_add(times.0.len() as u64)
                .saturating_add(times.1.len() as u64);
            clip_times.push(times);
        }
        let per_sample_cost = per_sample_work_units(source, &affected_skin_instances);
        check_sampling_budget(&tol, sample_times, per_sample_cost)?;

        for (clip_index, clip) in source.clips.iter().enumerate() {
            let candidate_clip =
                candidate
                    .clips
                    .get(clip_index)
                    .ok_or(ScaleError::MissingProofEvidence {
                        kind: ProofResidualKind::KeyTranslation,
                        detail: "candidate_clip_missing",
                    })?;
            let (key_times, interior_times) = &clip_times[clip_index];
            for &t in key_times {
                proof.sample_time_count += 1;
                if plan.has_key_translations() {
                    check_track_value_residual(
                        ProofResidualKind::KeyTranslation,
                        source,
                        clip,
                        candidate_clip,
                        &affected,
                        t,
                        plan,
                        &tol,
                        &mut proof,
                    )?;
                }
                sample_time_obligations(
                    source,
                    candidate,
                    clip,
                    candidate_clip,
                    t,
                    &affected_skin_instances,
                    plan,
                    &tol,
                    &mut proof,
                )?;
            }
            for &t in interior_times {
                proof.sample_time_count += 1;
                if plan.has_cubic_interiors() {
                    check_track_value_residual(
                        ProofResidualKind::CubicInterior,
                        source,
                        clip,
                        candidate_clip,
                        &affected,
                        t,
                        plan,
                        &tol,
                        &mut proof,
                    )?;
                }
                sample_time_obligations(
                    source,
                    candidate,
                    clip,
                    candidate_clip,
                    t,
                    &affected_skin_instances,
                    plan,
                    &tol,
                    &mut proof,
                )?;
            }
        }
    }

    check_rewritten_source_field_dispositions(
        source,
        candidate,
        plan,
        &affected,
        &tol,
        &mut discharged_field_rows,
    )?;
    check_preserved_field_dispositions(source, candidate, plan, &mut discharged_field_rows)?;
    finish_field_row_discharge(plan, &discharged_field_rows)?;
    Ok(proof)
}

/// Every obligation one sample time owes, evaluated against **one** pair of
/// world-matrix arrays.
///
/// The trajectory, skin, and bounds obligations all need the same source and
/// candidate poses at `t`. Deriving them once here — rather than letting each
/// obligation call [`world_at_time`] for itself, which recomputed forward
/// kinematics up to three times per sample per document — is what keeps the
/// proof's cost linear in the sample count rather than a fixed multiple of
/// it, and is a precondition for the sampling budget in [`prove_scale`] being
/// a meaningful bound on real work.
#[allow(clippy::too_many_arguments)]
fn sample_time_obligations(
    source: &Document,
    candidate: &Document,
    source_clip: &Clip,
    candidate_clip: &Clip,
    t: f32,
    affected_skin_instances: &[usize],
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    if plan.trajectory_nodes().is_none() && !plan.has_skin_and_bounds() {
        return Ok(());
    }
    let source_worlds = world_at_time(&source.skeleton, source_clip, t)?;
    let candidate_worlds = world_at_time(&candidate.skeleton, candidate_clip, t)?;
    if let Some(nodes) = plan.trajectory_nodes() {
        check_trajectory_residual_at(&source_worlds, &candidate_worlds, nodes, plan, tol, proof)?;
    }
    check_skin_and_bounds(
        source,
        candidate,
        &source_worlds,
        &candidate_worlds,
        affected_skin_instances,
        plan,
        tol,
        proof,
    )
}

/// The two document sides — source and candidate — every sampled obligation
/// walks. [`sample_time_obligations`] poses both skeletons, and
/// [`check_skin_and_bounds`] resolves both slot palettes and skins both
/// vertex arrays, so every term of [`per_sample_work_units`] is charged twice.
const PROOF_SIDES: u64 = 2;

/// Refuse a document whose total sampled work exceeds
/// [`ScaleTolerancePolicy::proof_sample_work_budget`], before the first
/// sample time is evaluated.
///
/// A free function rather than an inline comparison in [`prove_scale`] so
/// that the boundary itself is directly testable on synthetic numbers. The
/// budget is a ceiling the document may *reach*: the comparison is `>`, not
/// `>=`, exactly as [`check_residual`]'s is, and for the same reason —
/// DESIGN.md Appendix D §D.1 states every policy quantity as an inclusive
/// "at most". Pinning that end to end would mean a document that then costs
/// `1e8` work units to prove; pinning it here costs nothing and asserts the
/// same thing.
///
/// # Errors
///
/// Returns [`ScaleError::ProofSamplingBudgetExceeded`] carrying both factors
/// and the product, so the caller can see which of the two is oversized.
pub(super) fn check_sampling_budget(
    tol: &ScaleTolerancePolicy,
    sample_times: u64,
    per_sample_cost: u64,
) -> Result<(), ScaleError> {
    let work = sample_times.saturating_mul(per_sample_cost);
    if work > tol.proof_sample_work_budget {
        return Err(ScaleError::ProofSamplingBudgetExceeded {
            policy_id: tol.id,
            sample_times,
            per_sample_cost,
            work,
            budget: tol.proof_sample_work_budget,
        });
    }
    Ok(())
}

/// Work units one sample time costs, for
/// [`ScaleTolerancePolicy::proof_sample_work_budget`].
///
/// The charge is what [`sample_time_obligations`] and
/// [`check_skin_and_bounds`] actually perform at one sample time, term by
/// term. Everything they walk, they walk for **both** document sides, so
/// every term below carries the [`PROOF_SIDES`] factor:
///
/// - one forward-kinematics pass over the skeleton per side, owed by every
///   sampled obligation — hence `2 * bone_count`, always charged. Only the
///   *source* skeleton is measured here, which is sound only because
///   [`validate_candidate_structure`] has already rejected a candidate whose
///   bone count differs; see the note on its `bone_count_mismatch` clause for
///   what an unchecked candidate skeleton cost;
/// - per affected skinned instance, one `world * inverse_bind` product per
///   [`crate::model::MeshInstance::skin_joints`] slot per side, plus one residual
///   comparison per slot when the skin obligation is declared; and
/// - per affected skinned instance, every vertex of **every** primitive of
///   its mesh per side, when the bounds obligation is declared.
///
/// The slot term is charged explicitly because nothing bounds it and nothing
/// else stands in for it. An earlier revision charged only `bone_count +
/// vertices` on the claim that slot work "cannot exceed the bone count",
/// which is false twice over: [`validate_scale_input`] only range-checks
/// joint ids, so `skin_joints` may repeat a joint and be arbitrarily long,
/// and the instance count is unbounded, so the total is
/// `sum over instances of len(skin_joints)` with no relation to
/// `bone_count` at all. A legal 400-instance document with 300 slots each and
/// one vertex per instance was charged `120_600` while performing `36_000_000`
/// slot matrix products — a `299x` undercount, and unbounded in general.
///
/// This bounds the *sampled* work, which is what grows with the document's
/// key count. [`prove_scale`] additionally evaluates the rest pose once,
/// outside the sampled loop and outside this budget; that is one extra pose
/// of the same shape, not a term that scales with anything.
pub(super) fn per_sample_work_units(document: &Document, affected_skin_instances: &[usize]) -> u64 {
    let mut units = PROOF_SIDES.saturating_mul(document.skeleton.bones.len() as u64);
    for &instance_index in affected_skin_instances {
        let instance = &document.assets.instances[instance_index];
        let slots = instance.skin_joints.len() as u64;
        units = units.saturating_add(PROOF_SIDES.saturating_mul(slots));
        units = units.saturating_add(slots);
        let Some(mesh) = document.assets.meshes.get(instance.mesh) else {
            continue;
        };
        for primitive in &mesh.primitives {
            units =
                units.saturating_add(PROOF_SIDES.saturating_mul(primitive.positions.len() as u64));
        }
    }
    units
}

/// Fail closed on any residual that is not provably within `tolerance`.
///
/// The non-finite guard is load-bearing, not defensive noise: `NaN > x` is
/// `false` for every `x`, so a bare `observed > tolerance` reports a `NaN`
/// residual — the exact signature of a candidate built with an overflowing
/// factor, or of a comparison against a non-finite source value — as a pass.
/// A `NaN` tolerance (from a non-finite before/after magnitude) fails the
/// same way and is rejected for the same reason.
///
/// The guard is `!observed.is_finite()` rather than `observed.is_nan()`, and
/// the difference is narrower than it looks: `+inf > tolerance` is true, so
/// the comparison alone already rejects a positive infinity. Only a
/// *negative* non-finite residual needs the wider guard, and no caller in
/// this module can produce one — every `observed` here is an `abs()`, a
/// `length()`, or a `max` fold over those. The wider spelling is kept because
/// this function's contract is the fail-closed one stated above rather than
/// "whatever today's callers happen to pass", and it is pinned by
/// `a_non_finite_residual_fails_closed_instead_of_comparing_false`.
pub(super) fn check_residual(
    kind: ProofResidualKind,
    observed: f64,
    tolerance: f64,
) -> Result<(), ScaleError> {
    if !observed.is_finite() || !tolerance.is_finite() || observed > tolerance {
        return Err(ScaleError::ProofResidualExceeded {
            kind,
            observed,
            tolerance,
        });
    }
    Ok(())
}

impl ScaleProof {
    /// Record the raw ulp demand of one f32-rounded comparison at the exact
    /// point its residual and rounding base meet.
    ///
    /// This is calibration instrumentation, not published evidence. A zero
    /// base and zero residual make no demand on the rounding count; a nonzero
    /// residual with no provenance records infinity so calibration fails
    /// closed instead of silently reporting zero.
    #[cfg(test)]
    pub(super) fn record_f32_rounding_demand(
        &mut self,
        kind: ProofResidualKind,
        observed: f64,
        magnitude: f64,
    ) {
        let demand = if magnitude > 0.0 {
            observed / (magnitude * f64::from(f32::EPSILON))
        } else if observed == 0.0 {
            0.0
        } else {
            f64::INFINITY
        };
        let slot = match kind {
            ProofResidualKind::RestTranslation => &mut self.rest_translation_f32_rounding_demand,
            ProofResidualKind::Trajectory => &mut self.trajectory_f32_rounding_demand,
            ProofResidualKind::SkinMatrix => &mut self.skin_matrix_f32_rounding_demand,
            ProofResidualKind::Bounds => &mut self.bounds_f32_rounding_demand,
            ProofResidualKind::UnaffectedInverseBind => {
                &mut self.unaffected_inverse_bind_f32_rounding_demand
            }
            _ => unreachable!("only f32-rounded residual kinds record a raw rounding demand"),
        };
        *slot = slot.max(demand);
    }

    /// The maximum/count pair this residual kind reports into.
    ///
    /// The single mapping from a [`ProofResidualKind`] to the fields it
    /// writes. Every comparison site names its kind and nothing else, so a
    /// site cannot report one kind's residual into another kind's field —
    /// which was previously possible wherever a `&mut f64` and a `kind` were
    /// passed as independent arguments.
    ///
    /// [`ProofResidualKind::ObservedFactor`] is not a residual — it names a
    /// source whose scaled root could not be resolved, and is only ever
    /// reported as [`ScaleError::MissingProofEvidence`] — so it has no pair
    /// and no comparison site reaches here with it.
    fn tally(&mut self, kind: ProofResidualKind) -> Option<&mut ScaleProofResidual> {
        let tally = match kind {
            ProofResidualKind::RestTranslation => &mut self.rest_translation,
            ProofResidualKind::RestRotation => &mut self.rest_rotation,
            ProofResidualKind::UnitScale => &mut self.unit_scale,
            ProofResidualKind::TransformOnlyAffine => &mut self.transform_only_affine,
            ProofResidualKind::TrackValue => &mut self.track_value,
            ProofResidualKind::MeshPosition => &mut self.mesh_position,
            ProofResidualKind::KeyTranslation => &mut self.key_translation,
            ProofResidualKind::CubicInterior => &mut self.cubic_interior,
            ProofResidualKind::Trajectory => &mut self.trajectory,
            ProofResidualKind::SkinMatrix => &mut self.skin_matrix,
            ProofResidualKind::Bounds => &mut self.bounds,
            ProofResidualKind::UnaffectedInverseBind => &mut self.unaffected_inverse_bind,
            ProofResidualKind::ObservedFactor => return None,
        };
        Some(tally)
    }
}

/// Record one comparison of `observed` for `kind` and check it against
/// `tolerance`.
///
/// The single point at which a residual maximum moves. Recording and
/// checking here — rather than at each of the twelve obligations' loops —
/// is what makes [`ScaleProof`]'s counts describe exactly the comparisons
/// its maxima were taken over.
fn record_and_check(
    kind: ProofResidualKind,
    observed: f64,
    tolerance: f64,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    if let Some(tally) = proof.tally(kind) {
        tally.record(observed);
    }
    check_residual(kind, observed, tolerance)
}

/// Record `observed` for `kind` and check it against the
/// before/after-derived tolerance for this specific comparison — never a
/// proxy such as the plan's declared factor.
fn check_and_track(
    kind: ProofResidualKind,
    observed: f64,
    before: f64,
    after: f64,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    record_and_check(kind, observed, tol.scalar_tolerance(before, after), proof)
}

/// [`check_and_track`] for a residual between two `f32`-rounded quantities,
/// carrying the `magnitude` their arithmetic actually ran on.
///
/// Only the five obligations whose compared quantity can be made arbitrarily
/// smaller than that magnitude by a rotation use this — see
/// [`ScaleTolerancePolicy::f32_rounding_ulps`]. Every other obligation
/// compares a vector length or a matrix entry against its own magnitude,
/// where the two are the same number and the extra term would be noise.
fn check_and_track_f32_rounded(
    kind: ProofResidualKind,
    observed: f64,
    before: f64,
    after: f64,
    magnitude: f64,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    #[cfg(test)]
    proof.record_f32_rounding_demand(kind, observed, magnitude);
    record_and_check(
        kind,
        observed,
        tol.f32_rounded_tolerance(before, after, magnitude),
        proof,
    )
}

/// Rest-world translation residual for one node, plus the expected/actual
/// translation magnitudes the caller uses to derive this comparison's own
/// tolerance.
///
/// This deliberately does not also report a rotation residual: extracting a
/// rotation via [`Mat4::to_scale_rotation_translation`] out of a world
/// matrix whose linear part mixes a small uniform scale with an actual
/// rotation is numerically fragile in `f32`, and unnecessary here — both
/// operations leave every node's *local* rotation field untouched, so
/// callers that need a rotation residual should compare
/// [`crate::model::Bone::rest`]`.rotation` directly (see [`prove_scale`]),
/// which is both exact and, by composition, implies preserved world
/// orientation.
fn rest_node_residual(
    before: Mat4,
    after: Mat4,
    whole_document: bool,
    factor: f64,
) -> (f64, f64, f64) {
    let (_, _, before_translation) = before.to_scale_rotation_translation();
    let (_, _, after_translation) = after.to_scale_rotation_translation();
    let expected_translation = if whole_document {
        before_translation.as_dvec3() * factor
    } else {
        before_translation.as_dvec3()
    };
    let actual_translation = after_translation.as_dvec3();
    let translation_residual = (actual_translation - expected_translation).length();
    (
        translation_residual,
        expected_translation.length(),
        actual_translation.length(),
    )
}

/// The multiplier this plan analytically expects a given node's translation
/// values to have been rewritten by.
///
/// Whole-document conversion multiplies every translation by the declared
/// factor. Rest/bind reparameterization multiplies by the target node's
/// *parent-basis* factor: the domain's common factor when the node's parent
/// is itself affected, the unaffected boundary factor of one otherwise.
fn translation_multiplier(
    document: &Document,
    node: BoneId,
    affected: &BTreeSet<BoneId>,
    plan: &ScalePlan,
) -> f64 {
    if plan.is_whole_document() {
        return plan.common_factor();
    }
    if !affected.contains(&node) {
        return 1.0;
    }
    match document
        .skeleton
        .bones
        .get(node)
        .and_then(|bone| bone.parent)
    {
        Some(parent) if affected.contains(&parent) => plan.common_factor(),
        _ => 1.0,
    }
}

/// Independently derive the scale-track boundary root the proof expects.
///
/// This deliberately does not call the reference writer's scale-animation
/// multiplier: the builder and proof must derive the selected-root boundary
/// separately, or a wrong builder helper could leave a root scale track
/// unchanged and teach the proof to accept it.
fn proof_scale_animation_root(
    source: &Document,
    plan: &ScalePlan,
) -> Result<Option<BoneId>, ScaleError> {
    let ScaleOperation::RestBindUniformScale {
        source_root_node_index,
        ..
    } = plan.operation()
    else {
        return Ok(None);
    };
    // Unlike the builder's parent/affected-boundary derivation, proof starts
    // from the operation's authored root selector and the source projection.
    // Agreement therefore requires two independent descriptions of which
    // bone owns the only non-unit local scale multiplier.
    let selected_root = source
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|asset| asset.source_node_index == source_root_node_index)
        .and_then(|asset| asset.bone)
        .ok_or(ScaleError::PlanDocumentMismatch {
            reason: "selected_root_projection_mismatch",
        })?;
    Ok(Some(selected_root))
}

/// Prove every retained per-element payload directly, not merely its shape.
///
/// [`validate_candidate_structure`] establishes that source and candidate
/// agree on clip/track/instance/mesh/primitive *counts* and on each track's
/// `(bone, property, interpolation, times)` identity — but it never looks
/// inside `values` or `positions`. Both are reachable through this module's
/// public API without any structural mismatch:
/// [`ScaleCandidate::from_document`] accepts an external document
/// independently of the source supplied to [`prove_scale`], so a doctored
/// candidate can be proved against the real source. Without a direct
/// comparison a rotation key rewritten from `0.1` to `2.5` radians, or an
/// interior mesh vertex moved anywhere at all, passes proof: the sampled
/// obligations only look at translation, world *joint* transforms, and the
/// bounding box's extreme vertices.
///
/// So every element of every domain is checked here against its analytic
/// expectation: rewritten domains against `before * multiplier` and
/// non-rewritten domains against `before` itself. Comparison is by
/// [`ScaleTolerancePolicy::scalar_tolerance`], never exact float equality,
/// which DESIGN.md Appendix D §D.1 forbids.
fn check_candidate_values(
    source: &Document,
    candidate: &Document,
    affected: &BTreeSet<BoneId>,
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
    discharged: &mut BTreeSet<usize>,
) -> Result<(), ScaleError> {
    let proof_scale_root = proof_scale_animation_root(source, plan)?;
    for (row_index, row) in plan.field_rows().iter().enumerate() {
        match row.target {
            ScaleFieldTarget::AnimationValues {
                clip_index,
                track_index,
                bone,
                property,
            } => {
                let track = &source.clips[clip_index].tracks[track_index];
                let candidate_track = &candidate.clips[clip_index].tracks[track_index];
                if track.bone != bone || track.property != property {
                    return Err(ScaleError::PlanDocumentMismatch {
                        reason: "compiled_animation_target_mismatch",
                    });
                }
                match (&track.values, &candidate_track.values) {
                    (TrackValues::Vec3s(before), TrackValues::Vec3s(after)) => {
                        let multiplier = match row.disposition {
                            ScaleFieldDisposition::PreserveExact => 1.0,
                            ScaleFieldDisposition::Rewrite(
                                ScaleRewriteRule::WholeDocumentLength,
                            ) => plan.common_factor(),
                            ScaleFieldDisposition::Rewrite(
                                ScaleRewriteRule::RestBindParentBasis,
                            ) => translation_multiplier(source, bone, affected, plan),
                            ScaleFieldDisposition::Rewrite(
                                ScaleRewriteRule::RestBindLocalScale,
                            ) => {
                                if proof_scale_root == Some(bone) {
                                    1.0 / plan.common_factor()
                                } else {
                                    1.0
                                }
                            }
                            ScaleFieldDisposition::Rewrite(
                                ScaleRewriteRule::RestBindNodeBasis
                                | ScaleRewriteRule::RestBindSourceLocal { .. },
                            ) => {
                                return Err(ScaleError::PlanDocumentMismatch {
                                    reason: "invalid_animation_rewrite_rule",
                                });
                            }
                        };
                        for (before, after) in before.iter().zip(after.iter()) {
                            let expected = before.as_dvec3() * multiplier;
                            let actual = after.as_dvec3();
                            let residual = (actual - expected).length();
                            check_and_track(
                                ProofResidualKind::TrackValue,
                                residual,
                                expected.length(),
                                actual.length(),
                                tol,
                                proof,
                            )?;
                        }
                    }
                    (TrackValues::Quats(before), TrackValues::Quats(after)) => {
                        for (before, after) in before.iter().zip(after.iter()) {
                            let residual = quat_equality_residual(*before, *after);
                            check_and_track(
                                ProofResidualKind::TrackValue,
                                residual,
                                before.length() as f64,
                                after.length() as f64,
                                tol,
                                proof,
                            )?;
                        }
                    }
                    _ => {
                        return Err(ScaleError::CandidateStructureMismatch {
                            reason: "track_value_variant_mismatch",
                        });
                    }
                }
                mark_field_row_discharged(discharged, row_index)?;
            }
            ScaleFieldTarget::MeshPositions {
                mesh_index,
                primitive_index,
            } => {
                // Base `POSITION` is proved per primitive, directly. Proving
                // it only through skinned bounds would miss interior vertices
                // and every unskinned instance.
                let source_primitive =
                    &source.assets.meshes[mesh_index].primitives[primitive_index];
                let candidate_primitive =
                    &candidate.assets.meshes[mesh_index].primitives[primitive_index];
                let position_multiplier = match row.disposition {
                    ScaleFieldDisposition::PreserveExact => 1.0,
                    ScaleFieldDisposition::Rewrite(ScaleRewriteRule::WholeDocumentLength) => {
                        plan.common_factor()
                    }
                    ScaleFieldDisposition::Rewrite(_) => {
                        return Err(ScaleError::PlanDocumentMismatch {
                            reason: "invalid_mesh_position_rewrite_rule",
                        });
                    }
                };
                for (before, after) in source_primitive
                    .positions
                    .iter()
                    .zip(candidate_primitive.positions.iter())
                {
                    let expected = before.as_dvec3() * position_multiplier;
                    let actual = after.as_dvec3();
                    let residual = (actual - expected).length();
                    check_and_track(
                        ProofResidualKind::MeshPosition,
                        residual,
                        expected.length(),
                        actual.length(),
                        tol,
                        proof,
                    )?;
                }
                mark_field_row_discharged(discharged, row_index)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Double-cover-aware component distance between two quaternion *values*,
/// computed in `f64`.
///
/// `q` and `-q` denote the same rotation, so the residual is the smaller of
/// the two component distances. Deliberately not an angle: nothing here
/// normalizes, divides, or takes an inverse cosine, so an authored
/// quaternion whose magnitude is not exactly one — which `invariant-9`
/// requires loaders to preserve — compares equal to an untouched copy of
/// itself at exactly `0.0` rather than at a magnitude-dependent artefact.
fn quat_equality_residual(before: Quat, after: Quat) -> f64 {
    let before = before.as_dquat();
    let after = after.as_dquat();
    (before - after).length().min((before + after).length())
}

/// Convert the chord length [`quat_equality_residual`] reports into the
/// shortest-path rotation angle it represents, in radians.
///
/// For unit quaternions `q1 . q2 = cos(theta / 2)`, so
/// `|q1 - q2|^2 = 2 - 2 * cos(theta / 2) = 4 * sin(theta / 4)^2` and the
/// chord is `2 * sin(theta / 4)`, which is `theta / 2` to first order.
/// Comparing that chord directly against
/// [`ScaleTolerancePolicy::rotation_residual_radians`] therefore accepted
/// *twice* the declared angle: a genuine `2e-5 rad` error measured
/// `9.99e-6` against a `1e-5` policy and passed. Inverting the relation
/// gives `theta = 4 * asin(chord / 2)`.
///
/// Converting here — rather than comparing in chord space, or renaming the
/// reported field to say "chord" — is the choice that keeps the public
/// evidence contract honest. DESIGN.md Appendix D §D.1 declares the bound as
/// "shortest-path rotation residual is at most `1e-5` radians", §D.6 requires
/// evidence to publish the tolerance policy *and* the observed residuals
/// together, and [`ScaleProof::rest_rotation`] is headed for the
/// immutable evidence format. A chord-valued residual sitting next to a
/// radian-valued policy in that record would hand every reader the same
/// factor-of-two misreading this conversion removes. Converting once, up
/// front, also keeps the comparison, the tracked maximum, and
/// [`ScaleError::ProofResidualExceeded`]'s `observed`/`tolerance` pair all in
/// one unit.
///
/// The conversion is monotone over the whole reachable chord range, so it
/// changes which residuals are accepted only by the intended factor of two,
/// never by re-ordering them. The clamp covers a chord above `2`, which no
/// pair of unit quaternions can produce (the double-cover minimum is at most
/// `sqrt(2)`) but an authored non-unit value can: saturating at `2 * pi`
/// fails closed on such a pair instead of reporting a `NaN` that only the
/// non-finite guard in [`check_residual`] would catch.
fn quat_residual_radians(chord: f64) -> f64 {
    4.0 * (chord / 2.0).min(1.0).asin()
}

/// Harvest the times every sampled obligation is evaluated at: every key
/// time of every animated track on an affected bone, plus the analytic
/// mid-segment interior of each cubic segment.
///
/// Deliberately *not* restricted to translation tracks. The sampled
/// obligations these times feed — trajectories, the skin equation, and
/// bounds — depend on a node's complete animated pose, so a clip that
/// animates an affected joint's rotation but not its translation would
/// otherwise yield zero sample times and make every sampled obligation
/// vacuously true while still reporting success.
fn clip_sample_times(clip: &Clip, affected: &BTreeSet<BoneId>) -> (Vec<f32>, Vec<f32>) {
    let mut keys = Vec::new();
    let mut interiors = Vec::new();
    for track in &clip.tracks {
        if !affected.contains(&track.bone) {
            continue;
        }
        keys.extend_from_slice(&track.times);
        if track.interpolation == Interpolation::CubicSpline {
            for window in track.times.windows(2) {
                interiors.push((window[0] + window[1]) * 0.5);
            }
        }
    }
    keys.sort_by(f32::total_cmp);
    keys.dedup();
    interiors.sort_by(f32::total_cmp);
    interiors.dedup();
    (keys, interiors)
}

#[allow(clippy::too_many_arguments)]
fn check_track_value_residual(
    kind: ProofResidualKind,
    source: &Document,
    source_clip: &Clip,
    candidate_clip: &Clip,
    affected: &BTreeSet<BoneId>,
    t: f32,
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    // Paired positionally, not by a `(bone, property)` lookup:
    // `validate_candidate_structure` already established that `source_clip`
    // and `candidate_clip` have the same track count and each pair agrees on
    // `(bone, property)`, so a positional pairing cannot silently match the
    // wrong duplicate the way a `find` could.
    for (track, candidate_track) in source_clip.tracks.iter().zip(candidate_clip.tracks.iter()) {
        if track.property != Property::Translation || !affected.contains(&track.bone) {
            continue;
        }
        let multiplier = translation_multiplier(source, track.bone, affected, plan);
        let TrackSample::Vec3(before) = sample_track(track, t) else {
            return Err(ScaleError::MissingProofEvidence {
                kind,
                detail: "source_sample_not_vec3",
            });
        };
        let TrackSample::Vec3(after) = sample_track(candidate_track, t) else {
            return Err(ScaleError::MissingProofEvidence {
                kind,
                detail: "candidate_sample_not_vec3",
            });
        };
        let expected = before.as_dvec3() * multiplier;
        let actual = after.as_dvec3();
        let residual = (actual - expected).length();
        check_and_track(
            kind,
            residual,
            expected.length(),
            actual.length(),
            tol,
            proof,
        )?;
    }
    Ok(())
}

/// Sampled world-space trajectory residual for one already-derived pose pair
/// (see [`sample_time_obligations`], which owns the single [`world_at_time`]
/// evaluation these matrices come from).
fn check_trajectory_residual_at(
    source_worlds: &WorldPose,
    candidate_worlds: &WorldPose,
    affected_nodes: &[BoneId],
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    for &node in affected_nodes {
        let before = source_worlds.bone(node)?.matrix;
        let after_pose = candidate_worlds.bone(node)?;
        let after = after_pose.matrix;
        let after_chain = after_pose.translation_rounding_magnitude;
        let (translation_residual, before_mag, after_mag) = rest_node_residual(
            before,
            after,
            plan.is_whole_document(),
            plan.common_factor(),
        );
        // The same magnitude the unanimated `RestTranslation` comparison
        // takes — the *candidate's* sampled parent chain, read off the sampled
        // pose this residual was composed from rather than the rest pose: the
        // two obligations differ only in which locals the chain ran on, and
        // the argument for reading the candidate side alone is the one stated
        // there.
        //
        // Reaching this term at all needs a rig whose *sampled* parent chain
        // cancels, which a clip over a rest pose that already cancels is the
        // simplest way to build:
        // `a_sampled_pose_whose_parent_chain_cancels_still_proves_its_trajectory`
        // and `the_trajectory_v6_floor_is_an_adjacent_f32_transition`
        // are those fixtures, and
        // `a_shrinking_conversion_holds_trajectory_to_the_candidate_s_own_chain`
        // is the one that separates the candidate's chain from the source's.
        // Without such a rig the only comparison this obligation makes has
        // `chain = 0` on both sides, and every mutation of the term is a no-op
        // on a quantity that is arithmetically absent.
        check_and_track_f32_rounded(
            ProofResidualKind::Trajectory,
            translation_residual,
            before_mag,
            after_mag,
            after_chain,
            tol,
            proof,
        )?;
    }
    Ok(())
}

/// Sample `clip` at `t` and compose parent-before-child world matrices,
/// validating every input before it is indexed or accumulated: an
/// out-of-range track bone rejects rather than being skipped, a non-finite
/// sampled value or accumulated matrix rejects, and a parent index that is
/// not strictly earlier than its child rejects — the same structural
/// invariant [`crate::model::world_rest_matrices`]
/// enforces for the unanimated rest pose.
pub(super) fn world_at_time(
    skeleton: &Skeleton,
    clip: &Clip,
    t: f32,
) -> Result<WorldPose, ScaleError> {
    let bone_count = skeleton.bones.len();
    let mut locals = vec![Transform::IDENTITY; bone_count];
    for (index, bone) in skeleton.bones.iter().enumerate() {
        locals[index] = bone.rest;
    }
    for track in &clip.tracks {
        if track.bone >= bone_count {
            return Err(ScaleError::BoneIndexOutOfRange { index: track.bone });
        }
        match sample_track(track, t) {
            TrackSample::Vec3(value) => {
                if !value.is_finite() {
                    return Err(ScaleError::NonFiniteTransform { node: track.bone });
                }
                match track.property {
                    Property::Translation => locals[track.bone].translation = value,
                    Property::Scale => locals[track.bone].scale = value,
                    Property::Rotation => {}
                }
            }
            TrackSample::Quat(value) => {
                if !value.is_finite() {
                    return Err(ScaleError::NonFiniteTransform { node: track.bone });
                }
                locals[track.bone].rotation = value;
            }
        }
    }
    let mut bones: Vec<WorldBonePose> = Vec::with_capacity(bone_count);
    for (index, bone) in skeleton.bones.iter().enumerate() {
        let local = locals[index].to_mat4();
        if !mat4_is_finite(local) {
            return Err(ScaleError::NonFiniteTransform { node: index });
        }
        let pose = match bone.parent {
            Some(parent) if parent < index => {
                // Accumulated in the same walk the composition runs in, so
                // the chain costs one `Mat4 * Vec4` per bone rather than a
                // second pass that would have to recompose every local.
                let parent_pose = bones[parent];
                let matrix = parent_pose.matrix * local;
                WorldBonePose {
                    matrix,
                    translation_rounding_magnitude: child_translation_rounding_magnitude(
                        parent_pose,
                        local,
                    ),
                }
            }
            Some(parent) => {
                return Err(ScaleError::InvalidParent {
                    node: index,
                    parent,
                });
            }
            None => WorldBonePose {
                matrix: local,
                translation_rounding_magnitude: 0.0,
            },
        };
        if !mat4_is_finite(pose.matrix) {
            return Err(ScaleError::NonFiniteTransform { node: index });
        }
        bones.push(pose);
    }
    Ok(WorldPose { bones })
}

/// `abs(planned - proved) / max(abs(planned), abs(proved))` — the divergence
/// between the two observed-factor witnesses, on the same comparison base
/// [`ScaleTolerancePolicy::relative`] uses.
///
/// Sharing that base is what makes the reported number commensurable with
/// [`ScaleTolerancePolicy::observed_factor_divergence_ceiling`], which is a
/// sum of two bands stated on it. The one division this spelling costs is
/// what a predicate would not pay, so the two are not bit-identical near the
/// ceiling; nothing here compares against it, so nothing depends on that.
///
/// No floor on the base, for [`ScaleTolerancePolicy::relative`]'s reasons,
/// and none is needed: `planned` is strictly positive under both operations —
/// a declared factor [`super::plan_scale`] range-checked, or an observed one
/// [`super::planning::classify_affine`] proved non-singular — so the base is at least
/// `planned` and never zero. `proved` carries no such guarantee: it is read
/// from whichever `source` [`prove_scale`] was handed, and a degenerate one
/// measuring exactly zero there reports a divergence of one rather than a
/// division by zero.
fn relative_divergence(planned: f64, proved: f64) -> f64 {
    (planned - proved).abs() / planned.abs().max(proved.abs())
}

/// Re-derive this operation's observed factor from `source`, without reading
/// [`ScalePlan::observed_factor`].
///
/// For [`ScaleOperation::RestBindUniformScale`] the scaled root is resolved
/// the same way `planning::plan_rest_bind` resolves it — by `source_root_node_index`
/// through `source`'s own source-node projection — and its factor is then
/// measured from the *normalized* skeleton's rest-world matrix rather than
/// from the raw projection planning classified. That makes this a genuinely
/// second witness: it reads different stored data, through a different
/// composition path, and it is computed from whichever `source` this call was
/// handed, which [`prove_scale`] does not require to be the document the plan
/// came from.
///
/// # The scaled root is the minimum [`BoneId`] in the closure
///
/// §D.6 defines the observed factor *at the scaled root*, and "the lowest
/// affected bone id" is the reading a source-node-space walk and a
/// `BoneId`-space walk could otherwise land on separately. Once
/// [`crate::model::validate_document_shape`] holds they are the same node, and no
/// document can distinguish them:
///
/// - every insertion [`super::validation::rest_bind_affected_closure`] makes is the root itself,
///   a node on the ancestor path from a joint *up to* the root, or a BFS
///   descendant of something already in the set — so the closure lies inside
///   `subtree(root)` in source-node space;
/// - chain agreement is a parent-preserving injection, so it carries
///   `subtree(root)` onto `subtree(bone(root))` in `BoneId` space;
/// - [`crate::model::world_rest_matrices`] refuses a skeleton in which a parent's id is not
///   strictly less than its child's, so every member of `subtree(b)` has an
///   id at least `b`.
///
/// The scaled root is therefore the strict minimum id in the closure, and
/// reading either one reports the same number. That is a consequence of the
/// agreement precondition rather than of this function: without it the two
/// readings genuinely differ, and the difference is a false proof rather than
/// a naming quibble.
///
/// It is deliberately *not* a re-run of [`super::planning::classify_affine`]. Re-classifying
/// here would add a fresh rejection path — a proof source outside the
/// supported affine class would fail with a domain error rather than the
/// residual that actually matters — for no gain: whether the source is in the
/// class is a planning question, already answered, and the quantity wanted
/// here is only the factor.
///
/// # Errors
///
/// [`ScaleError::MissingProofEvidence`] when the plan's scaled root has no
/// projection in `source` to measure, and [`ScaleError::BoneIndexOutOfRange`]
/// when it projects to a bone `source` does not have. Neither is silently
/// reported as a zero factor.
pub(super) fn observed_factor_from_source(
    source: &Document,
    source_worlds: &WorldPose,
    plan: &ScalePlan,
) -> Result<f64, ScaleError> {
    let ScaleOperation::RestBindUniformScale {
        source_root_node_index,
        ..
    } = plan.operation()
    else {
        // Whole-document conversion declares its factor rather than observing
        // it; see [`ScalePlan::observed_factor`].
        return Ok(plan.common_factor());
    };
    let bone = source_node_index_map(source)
        .get(&source_root_node_index)
        .and_then(|asset| asset.bone)
        .ok_or(ScaleError::MissingProofEvidence {
            kind: ProofResidualKind::ObservedFactor,
            detail: "scaled_root_not_projected",
        })?;
    let world = source_worlds.bone(bone)?.matrix;
    Ok(average_affine_axis_length(affine_axis_lengths(
        Mat3::from_mat4(world),
    )))
}

/// Prove that inverse-bind evidence *outside* the affected closure came
/// through unchanged.
///
/// [`check_skin_and_bounds`] skips an instance with no joint in the affected
/// closure entirely, and nothing else looked at one either — so a candidate
/// that rewrote an unrelated skin's `skin_ibms` proved `Ok`. Reachable
/// through the public API, since [`ScaleCandidate::from_document`] and
/// [`prove_scale`] do not require their two documents to be the same one.
///
/// Three cases, kept distinct on purpose (issue #296):
///
/// - both sides store a bind for the slot — compared directly, as the
///   arrays they are;
/// - exactly one side stores one — [`ScaleError::MissingProofEvidence`]. A
///   candidate that dropped an array (falling back to a different bind) or
///   materialized one where the source had none has changed the skin, and
///   there is nothing to compare it against;
/// - neither side stores one — nothing to compare, and nothing is claimed.
///
/// That third case is what makes this fail-*closed* rather than
/// fail-*everything*. Resolving both sides through [`instance_bind`] instead
/// would have been the obvious implementation and would newly reject a
/// document carrying an unrelated skin with no bind evidence at all
/// (`MissingInverseBind`) — a document the operation genuinely does not
/// touch. So the resolution here stops at what each document *stores*
/// ([`stored_instance_bind`]) and reports the evidence-free slot as out of
/// scope rather than as proven.
///
/// Scope, stated exactly: this compares the bind each side resolves *for a
/// slot*, in the module's own precedence order — the instance array first,
/// then the bone convenience value. A [`crate::model::Bone::inverse_bind`]
/// that is shadowed by a non-empty `skin_ibms` is not authority for any slot
/// and is not compared here, and neither is
/// [`crate::model::SourceSkinAsset::inverse_bind_accessor`], which is
/// read-side evidence about the input accessor rather than a bind either
/// planning or proof consumes.
///
/// The skip is `any`, not `all`, and that is load-bearing rather than
/// idiomatic. An instance with *some* joint in the affected closure belongs to
/// [`check_skin_and_bounds`], which checks `W * B` on both sides and expects
/// exactly the rewrite the reference rest/bind writer performs on those
/// slots. Holding such an instance to "binds unchanged" as well rejects this
/// module's own output — pinned by
/// `a_partially_affected_skin_stays_with_the_skin_obligation_that_owns_it`.
///
/// # Known trap: a semantics-preserving representation change is refused
///
/// The one-stored-side rule is stated over *stored* evidence, and there is a
/// case where the two sides store different things and mean the same thing.
/// A complete-coverage source skin whose
/// [`crate::model::SourceSkinAsset::inverse_bind_accessor`] is
/// [`crate::model::SourceInverseBindAccessorStatus::Absent`] licenses the format-defined
/// identity default, so [`instance_bind`] resolves a slot with no stored
/// evidence to [`Mat4::IDENTITY`]. A candidate that materializes that default
/// as an explicit `[IDENTITY]` array has changed nothing about the effective
/// bind — and is exactly what the reference rest/bind writer does for an
/// *affected* instance, deliberately — yet is refused here as
/// `MissingProofEvidence { source_slot_bind_missing }`, and the converse as
/// `candidate_slot_bind_missing`.
///
/// This is not reachable through the shipped pipeline: the reference
/// rest/bind writer materializes an array only for an instance with an
/// affected joint, and this obligation skips those. It is a trap for a future
/// rest/bind frontend that normalizes bind representation on the way out.
///
/// It is left refusing rather than relaxed, for two reasons. First, the
/// refusal is the behaviour DESIGN.md §D.6 now states outright ("a slot
/// exactly one side records is a rewritten skin and is refused"), so
/// admitting the representation change is a contract amendment rather than a
/// bug fix. Second, the narrow patch — resolving through [`instance_bind`]
/// only in the one-sided rows — leaves the three rows incoherent, because the
/// neither-stored row would then be the only one that does *not* consult the
/// format default it is entirely about. The coherent alternative is a
/// differently-shaped rule: resolve both sides through [`instance_bind`] and
/// treat [`ScaleError::MissingInverseBind`] on *both* sides as the
/// out-of-scope case, which preserves the fail-closed property the third row
/// exists for while comparing effective binds throughout. That is a design
/// change and belongs in its own issue.
///
/// Unconditional, like the per-element comparisons in
/// [`check_candidate_values`], though for its own reason: it is a structural
/// claim about payloads the plan declares *unaffected*, so there is no
/// obligation flag that could switch it off without also making the plan's
/// "unaffected" claim unfalsifiable. Base `POSITION` is unconditional on the
/// other ground — a whole-document plan *does* rewrite it, and its only other
/// witness reports zero for a document with no skinned instance.
fn check_unaffected_instance_binds(
    source: &Document,
    candidate: &Document,
    affected: &BTreeSet<BoneId>,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    // Zipped rather than indexed: `validate_candidate_structure` has already
    // proved the two instance lists have equal length and pairwise equal
    // `skin_joints`, so pairing them positionally needs no fallible lookup
    // and cannot silently drop a trailing instance.
    for (instance, candidate_instance) in source
        .assets
        .instances
        .iter()
        .zip(candidate.assets.instances.iter())
    {
        if instance
            .skin_joints
            .iter()
            .any(|joint| affected.contains(joint))
        {
            continue;
        }
        for (slot, &joint) in instance.skin_joints.iter().enumerate() {
            let before = stored_instance_bind(source, instance, slot, joint)?;
            let after = stored_instance_bind(candidate, candidate_instance, slot, joint)?;
            let (before, after) = match (before, after) {
                (None, None) => continue,
                (Some(_), None) => {
                    return Err(ScaleError::MissingProofEvidence {
                        kind: ProofResidualKind::UnaffectedInverseBind,
                        detail: "candidate_slot_bind_missing",
                    });
                }
                (None, Some(_)) => {
                    return Err(ScaleError::MissingProofEvidence {
                        kind: ProofResidualKind::UnaffectedInverseBind,
                        detail: "source_slot_bind_missing",
                    });
                }
                (Some(before), Some(after)) => (before, after),
            };
            // Any slot reaching this function is outside a rest/bind closure
            // and therefore unchanged. A valid whole-document plan covers
            // every current bone; `validate_plan_document_inventory` rejects
            // stale replay before an added bone could reach this walk.
            let expected = before;
            let residual = matrix_residual(expected, after);
            // Both sides are stored matrices, so the magnitude the
            // comparison rounded against *is* the magnitude being compared:
            // `scale_translation_only` scales a column, it does not cancel
            // two terms the way composing `W * B` does, and a rotation
            // cannot make one of these entries small while its error stays
            // large. The rounding term is passed for the same base the
            // relative band already uses, which is what makes it inert here
            // — measured at `0` ulps across the whole rotation sweep,
            // because a candidate this obligation reads was produced by the
            // identical `f32` expression on the identical stored inputs. It
            // is stated rather than omitted so the policy quantity means one
            // thing across every obligation that compares `f32` matrices.
            let magnitude = matrix_magnitude(expected).max(matrix_magnitude(after));
            check_and_track_f32_rounded(
                ProofResidualKind::UnaffectedInverseBind,
                residual,
                matrix_magnitude(expected),
                matrix_magnitude(after),
                magnitude,
                tol,
                proof,
            )?;
        }
    }
    Ok(())
}

/// The skin-equation and skinned-bounds obligations, evaluated in **one**
/// walk over the affected skinned instances.
///
/// Both obligations need the same three things per instance: the instance's
/// joint world matrices, its resolved inverse binds, and — for bounds — its
/// vertices. Splitting them across two entry points meant resolving the binds
/// twice and, worse, walking every vertex twice for bounds alone (once for
/// the source document, once for the candidate). This walks each vertex once
/// and skins it through both sides.
///
/// [`validate_candidate_structure`] has already established that paired
/// instances agree on `mesh` and `skin_joints` and that paired meshes agree
/// on primitive and vertex counts, so the two sides are known to have the
/// same slots and the same vertices to walk.
#[allow(clippy::too_many_arguments)]
fn check_skin_and_bounds(
    source: &Document,
    candidate: &Document,
    source_worlds: &WorldPose,
    candidate_worlds: &WorldPose,
    affected_skin_instances: &[usize],
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    if !plan.has_skin_and_bounds() {
        return Ok(());
    }

    let mut source_bounds = BoundsAccumulator::default();
    let mut candidate_bounds = BoundsAccumulator::default();

    // The factor the source side is rebased by before it is compared, and so
    // the factor its *rounding* is rebased by too. Both obligations below take
    // their comparison base as `candidate.max(q * source)` for this reason —
    // see the note at the skin-matrix call. `1.0` for rest/bind, where the two
    // documents state the same world in the same units and the rebasing is a
    // no-op.
    let q = if plan.is_whole_document() {
        plan.common_factor()
    } else {
        1.0
    };

    for &instance_index in affected_skin_instances {
        let instance = &source.assets.instances[instance_index];
        let candidate_instance = candidate.assets.instances.get(instance_index).ok_or(
            ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::SkinMatrix,
                detail: "candidate_instance_missing",
            },
        )?;

        // Resolved once and reused by both obligations.
        let mut source_slots = Vec::with_capacity(instance.skin_joints.len());
        let mut candidate_slots = Vec::with_capacity(instance.skin_joints.len());
        for (slot, &joint) in instance.skin_joints.iter().enumerate() {
            let before_pose = source_worlds.bone(joint)?;
            let after_pose = candidate_worlds.bone(joint)?;
            let before_world = before_pose.matrix;
            let before_chain = before_pose.translation_rounding_magnitude;
            let after_world = after_pose.matrix;
            let after_chain = after_pose.translation_rounding_magnitude;
            let before_ibm = instance_bind(source, instance, slot, joint)?;
            let after_ibm = instance_bind(candidate, candidate_instance, slot, joint)?;
            source_slots.push(SkinSlot::compose(before_world, before_ibm, before_chain));
            candidate_slots.push(SkinSlot::compose(after_world, after_ibm, after_chain));
        }

        for (before, after) in source_slots.iter().zip(candidate_slots.iter()) {
            // Whole-document conversion scales every affine's translation
            // by the declared factor while leaving its linear part
            // unchanged (the same `U M U^-1` conjugation as any other
            // retained matrix); rest/bind reparameterization analytically
            // preserves the skin equation exactly.
            let expected = if plan.is_whole_document() {
                scale_translation_only(before.matrix, plan.common_factor() as f32)
            } else {
                before.matrix
            };
            let residual = matrix_residual(expected, after.matrix);
            // The candidate's own composition magnitude, and the source's
            // *rebased by the factor*. The residual is `|after - q *
            // before|`, so the source operand enters the comparison
            // multiplied by `q` and its rounding is multiplied by `q` with
            // it: a source slot accurate to `k` ulps of its own magnitude
            // contributes `q * k` ulps of that magnitude here. A base that
            // reads the source side unrebased — which `max(before, after)`
            // did — therefore states the source's error in the wrong units
            // by a factor of `q`.
            //
            // Under a *shrinking* conversion that is the whole defect: the
            // unrebased source magnitude is `1/q` times too large, so the
            // band freezes at the source rig's size while the candidate it
            // is spent on keeps shrinking. Measured over the sweep
            // populations below the recovered discriminating power is the
            // factor exactly — `100x` at `0.01`, `10_000x` at `1e-4`.
            //
            // Unlike the parent-chain case this does **not** reduce to the
            // candidate's magnitude alone, because `q * before` is not
            // bounded by `after`: the two magnitudes are a factor apart
            // only in the terms that carry a translation, and both retain
            // an unscaled `O(1)` floor from the composition's linear block
            // and the homogeneous row. Where that floor dominates the
            // source — small joints carrying small geometry — `q * before`
            // exceeds `after` by up to the full factor under a growing
            // conversion.
            //
            // `q * before` is written anyway because it is the bound that
            // can be argued from the operands rather than measured from a
            // population: a source slot accurate to the count's own budget
            // contributes `q` times that budget here, whatever cancels.
            // `a_growing_conversion_provisions_a_rebased_source_magnitude`
            // pins the regime where this rebased source term exceeds the
            // candidate term by orders of magnitude.
            let magnitude = after.rounding_magnitude.max(q * before.rounding_magnitude);
            check_and_track_f32_rounded(
                ProofResidualKind::SkinMatrix,
                residual,
                matrix_magnitude(expected),
                matrix_magnitude(after.matrix),
                magnitude,
                tol,
                proof,
            )?;
        }

        let mesh = source.assets.meshes.get(instance.mesh).ok_or(
            DocumentShapeError::MeshInstanceShape {
                instance_index,
                violation: MeshInstanceShapeViolation::MeshIndexOutOfRange,
            },
        )?;
        let candidate_mesh = candidate.assets.meshes.get(candidate_instance.mesh).ok_or(
            DocumentShapeError::MeshInstanceShape {
                instance_index,
                violation: MeshInstanceShapeViolation::MeshIndexOutOfRange,
            },
        )?;
        for (primitive_index, (primitive, candidate_primitive)) in mesh
            .primitives
            .iter()
            .zip(candidate_mesh.primitives.iter())
            .enumerate()
        {
            accumulate_skinned_bounds(
                instance_index,
                primitive_index,
                primitive,
                &source_slots,
                &mut source_bounds,
            )?;
            accumulate_skinned_bounds(
                instance_index,
                primitive_index,
                candidate_primitive,
                &candidate_slots,
                &mut candidate_bounds,
            )?;
        }
    }

    let source_bounds_magnitude = source_bounds.rounding_magnitude();
    let candidate_bounds_magnitude = candidate_bounds.rounding_magnitude();
    let (before_min, before_max) =
        source_bounds
            .finish()
            .ok_or(ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::Bounds,
                detail: "source_bounds_missing",
            })?;
    let (after_min, after_max) =
        candidate_bounds
            .finish()
            .ok_or(ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::Bounds,
                detail: "candidate_bounds_missing",
            })?;
    // One magnitude for all six comparisons, never the corner a residual lands
    // on: that corner is not evidence about the arithmetic that produced it. A
    // per-axis extreme is contributed by whichever vertex happened to be
    // furthest along that axis, and three vertices at `(3000, .001, .002)`,
    // `(.001, 3000, .003)` and `(.002, .003, 3000)` build a corner of magnitude
    // `2.4e-3` out of vertices of magnitude `3000` — so a base read off the
    // corner would be a million times smaller than the rounding error the
    // corner carries.
    //
    // The candidate's magnitude against the source's *rebased by the factor*,
    // for the reason the skin-matrix call states in full: the comparison below
    // is `|a - q * b|`, so the source bound's rounding enters it multiplied by
    // `q`. `max(source, candidate)` stated that rounding in the source rig's
    // units and was loose by `1/q` under a shrinking conversion — `100x` at
    // `0.01` and `10_000x` at `1e-4`, both recovered here.
    let magnitude = candidate_bounds_magnitude.max(q * source_bounds_magnitude);
    for (before, after) in [(before_min, after_min), (before_max, after_max)] {
        let before = before.to_array();
        let after = after.to_array();
        for axis in 0..3 {
            let b = before[axis] as f64;
            let a = after[axis] as f64;
            let expected = b * q;
            let residual = (a - expected).abs();
            check_and_track_f32_rounded(
                ProofResidualKind::Bounds,
                residual,
                expected,
                a,
                magnitude,
                tol,
                proof,
            )?;
        }
    }
    Ok(())
}

/// One skin slot's composed `W * B`, together with the magnitude that
/// composition rounded against.
///
/// The two travel together because a caller that has one without the other
/// cannot state a tolerance for anything derived from it: `matrix` is
/// near-identity for a bind-pose slot no matter how far from the origin the
/// joint sits, while `rounding_magnitude` is where the arithmetic actually
/// happened (see [`product_operand_magnitude`]).
#[derive(Debug, Clone, Copy)]
pub(super) struct SkinSlot {
    pub(super) matrix: Mat4,
    /// [`mat4_abs`] of [`Self::matrix`], for
    /// [`column_operand_magnitude`]'s per-vertex use.
    ///
    /// Held here rather than taken per vertex because it is constant across
    /// every vertex the slot influences and the loop that reads it is the
    /// hottest in this proof.
    pub(super) absolute: Mat4,
    pub(super) rounding_magnitude: f64,
}

impl SkinSlot {
    /// Compose `world * inverse_bind`, carrying the larger of the two
    /// magnitudes the result's error can come from.
    ///
    /// `world_translation_rounding_magnitude` is the accumulated provenance
    /// for this slot's joint. The fixed chain and `W * B` stages retain the
    /// measured policy's maximum here: #337 changes the unbounded number of
    /// links *inside* the incoming chain, not this fixed two-stage envelope.
    /// Both terms remain load-bearing in the calibrated corpus; replacing the
    /// envelope with an analytic componentwise error propagation is a broader
    /// model, not a larger scalar sum hidden in this constructor.
    pub(super) fn compose(
        world: Mat4,
        inverse_bind: Mat4,
        world_translation_rounding_magnitude: f64,
    ) -> Self {
        let matrix = world * inverse_bind;
        Self {
            matrix,
            absolute: mat4_abs(matrix),
            rounding_magnitude: product_operand_magnitude(world, inverse_bind)
                .max(world_translation_rounding_magnitude),
        }
    }
}

/// Running skinned-bounds extremes for one document side, and the largest
/// magnitude the `f32` arithmetic behind them ran on.
///
/// `touched` distinguishes "every relevant vertex was unweighted" from "the
/// bounds happen to be at the origin": the former has no bounds evidence at
/// all and must be reported as missing, not as a zero residual.
///
/// `rounding_magnitude` is what
/// [`ScaleTolerancePolicy::f32_rounding_ulps`] is counted in for
/// [`ProofResidualKind::Bounds`]. For each contributing influence, it is the
/// larger of the magnitude the `W * B * p` transform ran on and the slot's
/// [`SkinSlot::rounding_magnitude`]. The former is
/// [`column_operand_magnitude`] of the composed slot against `p` extended by
/// the homogeneous `1` — the *product* `abs(W * B) * abs(p)` and not either
/// factor alone. The latter carries the two earlier stages: the composition
/// that produced `W * B`, whose translation column may cancel large `W` and
/// `B` terms, and the parent chain that produced `W`, whose translation may
/// already contain cancellation.
///
/// The per-influence magnitudes are combined with the same binary64 weighted
/// average of the stored, non-negative binary32 weights as the skinned point.
/// A tiny influence therefore carries only its proportional arithmetic
/// provenance; taking a plain max would let an arbitrarily small weight on a
/// distant joint widen the whole bound tolerance. The blended point's own
/// per-axis magnitude is consequently already bounded by the weighted
/// transform operands and needs no separate L2 stage. See DESIGN.md Appendix
/// D §D.1.
pub(super) struct BoundsAccumulator {
    min: Vec3,
    max: Vec3,
    touched: bool,
    rounding_magnitude: f64,
}

impl Default for BoundsAccumulator {
    fn default() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
            touched: false,
            rounding_magnitude: 0.0,
        }
    }
}

impl BoundsAccumulator {
    pub(super) fn finish(self) -> Option<(Vec3, Vec3)> {
        self.touched.then_some((self.min, self.max))
    }

    pub(super) fn rounding_magnitude(&self) -> f64 {
        self.rounding_magnitude
    }
}

/// Skin one primitive's vertices through `slots` (already-composed
/// `W_i * B_i` per skin slot) and fold them into `bounds`, rejecting rather
/// than skipping every malformed input along the way: a primitive whose
/// per-vertex `joints`/`weights` are not exactly parallel to `positions`, a
/// non-finite position or weight, a joint-influence slot outside the
/// instance's `skin_joints`, or a non-finite skinned result. A vertex whose
/// four weights are all zero is legitimately unweighted (not malformed) and
/// is excluded from bounds.
pub(super) fn accumulate_skinned_bounds(
    instance_index: usize,
    primitive_index: usize,
    primitive: &Primitive,
    slots: &[SkinSlot],
    bounds: &mut BoundsAccumulator,
) -> Result<(), ScaleError> {
    if primitive.joints.len() != primitive.positions.len()
        || primitive.weights.len() != primitive.positions.len()
    {
        return Err(ScaleError::InvalidSkinnedPrimitive {
            instance_index,
            primitive_index,
            reason: "joints_or_weights_length_mismatch",
        });
    }
    for (vertex, &position) in primitive.positions.iter().enumerate() {
        if !position.is_finite() {
            return Err(ScaleError::InvalidSkinnedPrimitive {
                instance_index,
                primitive_index,
                reason: "non_finite_position",
            });
        }
        let joints = primitive.joints[vertex];
        let weights = primitive.weights[vertex];
        // Accumulate both the weighted numerator and denominator in binary64,
        // then narrow the normalized point once. Binary32 multiply-then-divide
        // can lose a lone subnormal contribution or overflow a large finite
        // denominator. Precomputing binary32 coefficients is not sufficient:
        // their rounded sum can exceed one and overflow an otherwise finite
        // convex blend at `f32::MAX`.
        let mut weight_sum = 0.0f64;
        for weight in weights {
            if weight == 0.0 {
                continue;
            }
            if !weight.is_finite() {
                return Err(ScaleError::InvalidSkinnedPrimitive {
                    instance_index,
                    primitive_index,
                    reason: "non_finite_weight",
                });
            }
            weight_sum += f64::from(weight);
        }
        if weight_sum == 0.0 {
            continue;
        }

        let mut skinned_numerator = DVec3::ZERO;
        let mut weighted_magnitude = 0.0f64;
        for slot_index in 0..4 {
            let stored_weight = weights[slot_index];
            if stored_weight == 0.0 {
                continue;
            }
            let Some(slot) = slots.get(joints[slot_index] as usize) else {
                return Err(ScaleError::InvalidSkinnedPrimitive {
                    instance_index,
                    primitive_index,
                    reason: "joint_influence_slot_out_of_range",
                });
            };
            let weight = f64::from(stored_weight);
            skinned_numerator += weight * slot.matrix.transform_point3(position).as_dvec3();
            // The magnitude `slot.matrix.transform_point3(position)` runs on,
            // which is `abs(W * B) * abs(p)` and not either factor alone.
            //
            // `abs(p)` alone — which is what this stage read before — names one
            // of the two. It is the right number only while `abs(W * B)` is
            // `1`, which is every rig whose slots are a pure rotation of the
            // bind pose, and that is the whole of what the fixtures below build:
            // `cancelling_blend_document` composes with `HALF_TURN_Z`, so its
            // stage was accidentally exact and no test could see the gap. Give
            // the composed slot a scale of `k` and the transform runs on
            // `k * abs(p)` while the base reads `abs(p)`, short by `k`; the
            // weighted sum over slots can then cancel the result to nothing
            // while every term still carries `k * abs(p)`'s ulp.
            // `two_slots_with_a_scaled_composition_cancel_a_vertex_and_still_prove_its_bounds`
            // is that rig, and it is refused outright — `observed: 1.53e-5`
            // against `tolerance: 8.63e-6` at `k = 16` — without this term.
            //
            // The homogeneous `1` is included because `transform_point3` sums
            // the translation column in with the rest, so that column's entries
            // are terms of the same dot product.
            let influence_magnitude = skin_influence_magnitude(slot, position);
            weighted_magnitude += weight * influence_magnitude;
        }
        let skinned = (skinned_numerator / weight_sum).as_vec3();
        if !skinned.is_finite() {
            // Overflow and `NaN` are different failures and are
            // reported as such: an overflowing skinned position is a
            // document whose geometry leaves the `f32` range this proof
            // computes in, while a `NaN` is a malformed or degenerate
            // input that survived every finiteness check above. No
            // magnitude domain is documented for the former, because the
            // boundary is not a property of any magnitude a document
            // could be checked against ahead of time:
            // `transform_point3` accumulates a dot product whose
            // intermediate terms depend on the rotation, so two rigs
            // whose skinned extents agree can disagree on whether they
            // compose finitely.
            return Err(ScaleError::InvalidSkinnedPrimitive {
                instance_index,
                primitive_index,
                reason: if skinned.is_nan() {
                    "non_finite_result"
                } else {
                    "skinned_magnitude_overflow"
                },
            });
        }
        bounds.min = bounds.min.min(skinned);
        bounds.max = bounds.max.max(skinned);
        let vertex_magnitude = weighted_magnitude / weight_sum;
        bounds.rounding_magnitude = bounds.rounding_magnitude.max(vertex_magnitude);
        bounds.touched = true;
    }
    Ok(())
}

/// The per-axis arithmetic provenance one weighted skin influence carries
/// into a bound. The transform application and the already-composed slot can
/// each dominate by an unbounded ratio, so the influence retains the larger;
/// [`accumulate_skinned_bounds`] then combines influences with the same
/// binary64 weighted average as the skinned point.
pub(super) fn skin_influence_magnitude(slot: &SkinSlot, position: Vec3) -> f64 {
    column_operand_magnitude(slot.absolute, position.extend(1.0)).max(slot.rounding_magnitude)
}
