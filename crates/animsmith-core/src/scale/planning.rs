//! Private scale planning, compiled-ledger construction, and replay validation.
//!
//! This module owns operation planning and the numeric-value-free structural
//! inventory a [`super::ScalePlan`] compiles. Candidate construction and proof
//! remain separate so their numeric expectations stay independently derived.

use super::validation::{
    derive_rest_bind_plan_domain, source_node_index_map, source_skin_payload_shapes,
    source_world_matrix, validate_scale_input, whole_document_source_topology,
};
use super::{
    RestBindParams, ScaleBoneRestField, ScaleCompiledPlan, ScaleError, ScaleFieldDisposition,
    ScaleFieldPlan, ScaleFieldTarget, ScaleLedger, ScaleOperation, ScalePayloadShapeRow, ScalePlan,
    ScaleProjectedRole, ScaleProofObligation, ScaleRequest, ScaleRewriteRule, ScaleSourceNodeKind,
    ScaleSourceRestField, ScaleSourceTopologyRow, ScaleTolerancePolicy, WholeDocumentParams,
};
use crate::model::{
    AffineDomainViolation, BoneId, Document, Interpolation, PositiveUniformAffineTolerance,
    Property, SourceNodeLocalRest, SourceSkeletonCoverage, classify_positive_uniform_affine,
};
use glam::Mat3;
use std::collections::{BTreeMap, BTreeSet};

/// Plan the caller-selected [`ScaleOperation`] against `request.document`.
///
/// Pure and fail-closed: this function only reads `request.document` and
/// `request.capability`. It never returns a plan for a factor that was not
/// declared or an affine domain outside the initial supported class.
///
/// # Errors
///
/// Returns a typed [`ScaleError`] for every unsupported factor, selector,
/// capability gap, affine domain, closure incompleteness, factor mismatch,
/// or invalid document shape.
pub fn plan_scale(request: &ScaleRequest<'_>) -> Result<ScalePlan, ScaleError> {
    if !request.capability.is_supported_for(request.operation) {
        return Err(ScaleError::IncompleteCapability);
    }
    validate_scale_input(request.document)?;
    match request.operation {
        ScaleOperation::WholeDocumentLinearUnits { factor } => {
            plan_whole_document(request.document, factor)
        }
        ScaleOperation::RestBindUniformScale {
            source_skin_index,
            source_root_node_index,
            expected_factor,
        } => plan_rest_bind(
            request.document,
            source_skin_index,
            source_root_node_index,
            expected_factor,
        ),
    }
}

/// Reject an `f64` factor whose `f32` image cannot carry it.
///
/// Every rewrite in this module narrows to `f32` at the model boundary
/// (`Vec3`/`Mat4` are `f32`), so a factor that only exists in `f64` is not a
/// conversion this operation can perform. Both failure modes are silent
/// without this check: overflow to `±inf` produces a document full of
/// `NaN`/`inf` that only proof catches, and underflow of a nonzero factor to
/// `0.0f32` annihilates every length in the document while every proof
/// residual stays exactly zero.
///
/// Checked at *plan* time, for both the declared factor and the reciprocal
/// `1 / factor` the rest/bind basis correction derives from it, so no
/// unrepresentable factor ever reaches a builder.
pub(in crate::scale) fn check_factor_narrows(
    declared: f64,
    factor: f64,
) -> Result<f32, ScaleError> {
    let narrowed = factor as f32;
    if !narrowed.is_finite() || (narrowed == 0.0 && factor != 0.0) {
        return Err(ScaleError::FactorNotRepresentable {
            declared,
            factor,
            narrowed,
        });
    }
    Ok(narrowed)
}

fn field_disposition(active: bool, rule: ScaleRewriteRule) -> ScaleFieldDisposition {
    if active {
        ScaleFieldDisposition::Rewrite(rule)
    } else {
        ScaleFieldDisposition::PreserveExact
    }
}

fn compile_payload_shapes(document: &Document) -> Vec<ScalePayloadShapeRow> {
    let source_authoritative =
        document.assets.source_skeleton.coverage == SourceSkeletonCoverage::Complete;
    let mut rows = vec![ScalePayloadShapeRow::Document {
        bone_count: document.skeleton.bones.len(),
        source_node_count: if source_authoritative {
            document.assets.source_skeleton.nodes.len()
        } else {
            0
        },
        source_coverage: document.assets.source_skeleton.coverage,
        clip_count: document.clips.len(),
        instance_count: document.assets.instances.len(),
        mesh_count: document.assets.meshes.len(),
    }];
    rows.extend(
        document
            .skeleton
            .bones
            .iter()
            .enumerate()
            .map(|(bone, value)| ScalePayloadShapeRow::Bone {
                bone,
                parent: value.parent,
            }),
    );
    rows.extend(source_skin_payload_shapes(document));
    for (clip_index, clip) in document.clips.iter().enumerate() {
        rows.push(ScalePayloadShapeRow::Clip {
            clip_index,
            track_count: clip.tracks.len(),
        });
        rows.extend(clip.tracks.iter().enumerate().map(|(track_index, track)| {
            ScalePayloadShapeRow::Track {
                clip_index,
                track_index,
                bone: track.bone,
                property: track.property,
                interpolation: track.interpolation,
                key_count: track.times.len(),
                value_count: track.values.len(),
            }
        }));
    }
    for (instance_index, instance) in document.assets.instances.iter().enumerate() {
        rows.push(ScalePayloadShapeRow::Instance {
            instance_index,
            node: instance.node,
            source_node_index: instance.source_node_index,
            mesh: instance.mesh,
            joint_count: instance.skin_joints.len(),
            inverse_bind_count: instance.skin_ibms.len(),
        });
        rows.extend(
            instance
                .skin_joints
                .iter()
                .enumerate()
                .map(|(slot, &joint)| ScalePayloadShapeRow::InstanceJoint {
                    instance_index,
                    slot,
                    joint,
                }),
        );
    }
    for (mesh_index, mesh) in document.assets.meshes.iter().enumerate() {
        rows.push(ScalePayloadShapeRow::Mesh {
            mesh_index,
            source_mesh_index: mesh.source_mesh_index,
            primitive_count: mesh.primitives.len(),
        });
        rows.extend(
            mesh.primitives
                .iter()
                .enumerate()
                .map(
                    |(primitive_index, primitive)| ScalePayloadShapeRow::Primitive {
                        mesh_index,
                        primitive_index,
                        position_count: primitive.positions.len(),
                        normal_count: primitive.normals.len(),
                        joint_count: primitive.joints.len(),
                        weight_count: primitive.weights.len(),
                    },
                ),
        );
    }
    rows
}

fn compile_scale_ledger(
    document: &Document,
    operation: ScaleOperation,
    affected_nodes: &[BoneId],
    transform_only_attachments: &[BoneId],
    topology: &[ScaleSourceTopologyRow],
) -> ScaleLedger {
    let affected: BTreeSet<_> = affected_nodes.iter().copied().collect();
    let factor = match operation {
        ScaleOperation::WholeDocumentLinearUnits { factor } => factor,
        ScaleOperation::RestBindUniformScale {
            expected_factor, ..
        } => expected_factor,
    };
    let factor_changes = factor != 1.0;
    let whole_document = matches!(operation, ScaleOperation::WholeDocumentLinearUnits { .. });
    let mut fields = Vec::new();

    for (bone, value) in document.skeleton.bones.iter().enumerate() {
        let in_domain = affected.contains(&bone);
        let parent_in_domain = value
            .parent
            .is_some_and(|parent| affected.contains(&parent));
        let translation = if whole_document {
            field_disposition(factor_changes, ScaleRewriteRule::WholeDocumentLength)
        } else {
            field_disposition(
                factor_changes && in_domain && parent_in_domain,
                ScaleRewriteRule::RestBindParentBasis,
            )
        };
        let scale = if whole_document {
            ScaleFieldDisposition::PreserveExact
        } else {
            field_disposition(
                factor_changes && in_domain && !parent_in_domain,
                ScaleRewriteRule::RestBindLocalScale,
            )
        };
        for (field, disposition) in [
            (ScaleBoneRestField::Translation, translation),
            (
                ScaleBoneRestField::Rotation,
                ScaleFieldDisposition::PreserveExact,
            ),
            (ScaleBoneRestField::Scale, scale),
        ] {
            fields.push(ScaleFieldPlan {
                target: ScaleFieldTarget::BoneRest { bone, field },
                disposition,
                element_count: 1,
            });
        }
        if value.inverse_bind.is_some() {
            fields.push(ScaleFieldPlan {
                target: ScaleFieldTarget::BoneInverseBind { bone },
                disposition: if whole_document {
                    field_disposition(factor_changes, ScaleRewriteRule::WholeDocumentLength)
                } else {
                    field_disposition(
                        factor_changes && in_domain,
                        ScaleRewriteRule::RestBindNodeBasis,
                    )
                },
                element_count: 1,
            });
        }
    }

    let source_nodes = source_node_index_map(document);
    for topology_row in topology {
        let node = source_nodes
            .get(&topology_row.source_node_index)
            .expect("validated topology row has a source node");
        let (role, connector_tail) = match topology_row.kind {
            ScaleSourceNodeKind::Projected {
                role,
                incoming_connector_tail,
                ..
            } => (Some(role), incoming_connector_tail),
            ScaleSourceNodeKind::Connector | ScaleSourceNodeKind::OutsideDomain { .. } => {
                (None, None)
            }
        };
        let parent_rewrite = factor_changes
            && (whole_document || role.is_some_and(|role| role != ScaleProjectedRole::Root));
        let local_rewrite = factor_changes && role == Some(ScaleProjectedRole::Root);
        let source_rule = if whole_document {
            ScaleRewriteRule::WholeDocumentLength
        } else {
            ScaleRewriteRule::RestBindSourceLocal { connector_tail }
        };
        let push = |fields: &mut Vec<ScaleFieldPlan>, field: ScaleSourceRestField, active: bool| {
            fields.push(ScaleFieldPlan {
                target: ScaleFieldTarget::SourceNodeRest {
                    source_node_index: node.source_node_index,
                    field,
                },
                disposition: field_disposition(active, source_rule),
                element_count: 1,
            });
        };
        match node.local_rest {
            SourceNodeLocalRest::Trs { .. } => {
                push(
                    &mut fields,
                    ScaleSourceRestField::Translation,
                    parent_rewrite,
                );
                push(&mut fields, ScaleSourceRestField::Rotation, false);
                push(&mut fields, ScaleSourceRestField::Scale, local_rewrite);
            }
            SourceNodeLocalRest::Matrix(_) => {
                push(
                    &mut fields,
                    ScaleSourceRestField::MatrixLinear,
                    local_rewrite,
                );
                push(
                    &mut fields,
                    ScaleSourceRestField::MatrixTranslation,
                    parent_rewrite,
                );
                push(&mut fields, ScaleSourceRestField::MatrixHomogeneous, false);
            }
        }
    }

    let mut has_tracks = false;
    for (clip_index, clip) in document.clips.iter().enumerate() {
        for (track_index, track) in clip.tracks.iter().enumerate() {
            has_tracks = true;
            let parent_in_domain = document
                .skeleton
                .bones
                .get(track.bone)
                .and_then(|bone| bone.parent)
                .is_some_and(|parent| affected.contains(&parent));
            let disposition = match track.property {
                Property::Translation if whole_document => {
                    field_disposition(factor_changes, ScaleRewriteRule::WholeDocumentLength)
                }
                Property::Translation if affected.contains(&track.bone) => field_disposition(
                    factor_changes && parent_in_domain,
                    ScaleRewriteRule::RestBindParentBasis,
                ),
                Property::Scale if !whole_document && affected.contains(&track.bone) => {
                    field_disposition(
                        factor_changes && !parent_in_domain,
                        ScaleRewriteRule::RestBindLocalScale,
                    )
                }
                _ => ScaleFieldDisposition::PreserveExact,
            };
            fields.push(ScaleFieldPlan {
                target: ScaleFieldTarget::AnimationValues {
                    clip_index,
                    track_index,
                    bone: track.bone,
                    property: track.property,
                },
                disposition,
                element_count: track.values.len(),
            });
        }
    }

    let mut has_affected_slots = false;
    let mut has_unaffected_slots = false;
    let mut has_skinned_instances = false;
    for (instance_index, instance) in document.assets.instances.iter().enumerate() {
        let instance_affected = instance
            .skin_joints
            .iter()
            .any(|joint| affected.contains(joint));
        if instance_affected {
            has_skinned_instances = true;
        }
        let slots: Vec<_> = if whole_document {
            instance
                .skin_ibms
                .iter()
                .enumerate()
                .filter_map(|(slot, _)| {
                    instance
                        .skin_joints
                        .get(slot)
                        .copied()
                        .map(|joint| (slot, joint))
                })
                .collect()
        } else {
            instance.skin_joints.iter().copied().enumerate().collect()
        };
        for (slot, joint) in slots {
            if whole_document || instance_affected {
                has_affected_slots = true;
            } else {
                has_unaffected_slots = true;
            }
            fields.push(ScaleFieldPlan {
                target: ScaleFieldTarget::InstanceInverseBind {
                    instance_index,
                    slot,
                    joint,
                },
                disposition: if whole_document {
                    field_disposition(factor_changes, ScaleRewriteRule::WholeDocumentLength)
                } else if instance_affected {
                    ScaleFieldDisposition::Rewrite(ScaleRewriteRule::RestBindNodeBasis)
                } else {
                    ScaleFieldDisposition::PreserveExact
                },
                element_count: 1,
            });
        }
    }

    let mut has_primitives = false;
    for (mesh_index, mesh) in document.assets.meshes.iter().enumerate() {
        for (primitive_index, primitive) in mesh.primitives.iter().enumerate() {
            has_primitives = true;
            fields.push(ScaleFieldPlan {
                target: ScaleFieldTarget::MeshPositions {
                    mesh_index,
                    primitive_index,
                },
                disposition: if whole_document {
                    field_disposition(factor_changes, ScaleRewriteRule::WholeDocumentLength)
                } else {
                    ScaleFieldDisposition::PreserveExact
                },
                element_count: primitive.positions.len(),
            });
            fields.push(ScaleFieldPlan {
                target: ScaleFieldTarget::MeshNormals {
                    mesh_index,
                    primitive_index,
                },
                disposition: ScaleFieldDisposition::PreserveExact,
                element_count: primitive.normals.len(),
            });
        }
    }

    let has_unaffected_nodes = affected.len() != document.skeleton.bones.len();
    let sampled = sampled_evidence(document, &affected);
    let has_connectors = topology
        .iter()
        .any(|row| matches!(row.kind, ScaleSourceNodeKind::Connector));
    let mut obligations = vec![
        ScaleProofObligation::ExactTopology,
        ScaleProofObligation::ExactPayloadIdentity,
    ];
    if has_unaffected_nodes {
        obligations.push(ScaleProofObligation::ExactUnchangedWorldRest);
    }
    if !affected_nodes.is_empty() {
        obligations.push(if whole_document {
            ScaleProofObligation::RestWorld
        } else {
            ScaleProofObligation::RestWorldAndUnitScale
        });
    }
    if !transform_only_attachments.is_empty() {
        obligations.push(ScaleProofObligation::TransformOnlyAffine);
    }
    if has_tracks {
        obligations.push(ScaleProofObligation::TrackValues);
    }
    if has_primitives {
        obligations.push(ScaleProofObligation::MeshPositions);
    }
    if sampled.key_translations {
        obligations.push(ScaleProofObligation::KeyTranslations);
    }
    if sampled.cubic_interiors {
        obligations.push(ScaleProofObligation::CubicInteriors);
    }
    if sampled.sample_times {
        obligations.push(ScaleProofObligation::Trajectories);
    }
    if has_skinned_instances {
        obligations.push(ScaleProofObligation::SkinAndBounds);
    }
    if has_affected_slots {
        obligations.push(ScaleProofObligation::AffectedInverseBinds);
    }
    if has_unaffected_slots {
        obligations.push(ScaleProofObligation::UnaffectedInverseBinds);
    }
    if has_connectors {
        obligations.push(ScaleProofObligation::ExactConnectorProjection);
    }
    ScaleLedger {
        field_rows: fields,
        payload_shapes: compile_payload_shapes(document),
        obligations,
    }
}

fn plan_whole_document(document: &Document, factor: f64) -> Result<ScalePlan, ScaleError> {
    if !factor.is_finite() || factor <= 0.0 {
        return Err(ScaleError::InvalidFactor { factor });
    }
    check_factor_narrows(factor, factor)?;
    let affected_nodes: Vec<BoneId> = (0..document.skeleton.bones.len()).collect();
    let source_topology = whole_document_source_topology(document);
    let ledger = compile_scale_ledger(
        document,
        ScaleOperation::WholeDocumentLinearUnits { factor },
        &affected_nodes,
        &[],
        &source_topology,
    );
    Ok(ScalePlan {
        tolerance_policy: ScaleTolerancePolicy::APPENDIX_D_V6,
        // Declared, not measured — see [`ScalePlan::observed_factor`].
        observed_factor: factor,
        affected_nodes,
        source_topology,
        ledger,
        compiled: ScaleCompiledPlan::WholeDocument(WholeDocumentParams { factor }),
    })
}

fn plan_rest_bind(
    document: &Document,
    source_skin_index: usize,
    source_root_node_index: usize,
    expected_factor: f64,
) -> Result<ScalePlan, ScaleError> {
    if !expected_factor.is_finite() || expected_factor <= 0.0 {
        return Err(ScaleError::InvalidExpectedFactor {
            factor: expected_factor,
        });
    }
    // Both directions: the builder narrows `expected_factor` itself, and the
    // proof's basis correction `C = scale(1 / s)` narrows its reciprocal.
    check_factor_narrows(expected_factor, expected_factor)?;
    check_factor_narrows(expected_factor, 1.0 / expected_factor)?;
    let domain = derive_rest_bind_plan_domain(document, source_skin_index, source_root_node_index)?;
    let by_source_index = source_node_index_map(document);
    let bone_of_source = domain.bone_of_source();
    let connector_sources = domain.connector_sources();

    let tol = ScaleTolerancePolicy::APPENDIX_D_V6;
    let mut world_cache = BTreeMap::new();
    let mut node_factor: BTreeMap<BoneId, f64> = BTreeMap::new();
    for (&source, &bone) in &bone_of_source {
        let world = source_world_matrix(
            source,
            &by_source_index,
            &connector_sources,
            &mut world_cache,
        )?;
        let linear = Mat3::from_mat4(world);
        let factor = classify_affine(linear, &tol)
            .map_err(|reason| ScaleError::InvalidAffineDomain { node: bone, reason })?;
        node_factor.insert(bone, factor);
    }
    // Read at the scaled root, which DESIGN.md Appendix D §D.6 names, rather
    // than at whichever affected node happens to sort first. The two readings
    // were separable before this module related its two parent chains; under
    // the source-projection agreement [`crate::model::validate_document_shape`]
    // establishes they are provably the same node, and the proof is written out on
    // `observed_factor_from_source` (§ "The scaled root is the minimum
    // BoneId in the closure"). Naming the root explicitly keeps this reading
    // and that second witness pinned to the same definition rather than to a
    // coincidence of ordering.
    let observed_common = node_factor[&domain.scaled_root_bone];
    if !tol.relative(tol.common_factor, observed_common, expected_factor) {
        return Err(ScaleError::FactorMismatch {
            expected: expected_factor,
            observed: observed_common,
        });
    }
    for (&bone, &factor) in &node_factor {
        if bone == domain.scaled_root_bone {
            continue;
        }
        if !tol.relative(tol.common_factor, factor, observed_common) {
            return Err(ScaleError::MixedFactor {
                expected: observed_common,
                observed: factor,
                node: bone,
            });
        }
    }

    let affected_nodes = domain.affected_nodes();
    let transform_only_attachments = domain.transform_only_attachments();
    let operation = ScaleOperation::RestBindUniformScale {
        source_skin_index,
        source_root_node_index,
        expected_factor,
    };
    let ledger = compile_scale_ledger(
        document,
        operation,
        &affected_nodes,
        &transform_only_attachments,
        &domain.source_rows,
    );
    Ok(ScalePlan {
        tolerance_policy: tol,
        // The measured source fact, kept alongside the declared factor the
        // build applies rather than discarded once it has been validated
        // against it (DESIGN.md Appendix D §D.6, "declared and observed
        // factors").
        observed_factor: observed_common,
        affected_nodes,
        source_topology: domain.source_rows,
        ledger,
        compiled: ScaleCompiledPlan::RestBind(RestBindParams {
            source_skin_index,
            source_root_node_index,
            expected_factor,
            transform_only_attachments,
        }),
    })
}

/// Re-derive the document-dependent part of `plan` against the source a
/// builder or proof was actually handed.
///
/// Numerical replay remains intentional: proof may independently observe
/// different transform values from those planning read. Structural replay is
/// different. Every proof loop is selected by the affected domain and the
/// evidence inventory, so accepting a source that derives a wider domain or
/// different obligations would let a stale plan omit payload altogether.
pub(in crate::scale) fn validate_plan_document_inventory(
    document: &Document,
    plan: &ScalePlan,
) -> Result<(), ScaleError> {
    validate_scale_input(document)?;
    let validate = |affected_nodes: &[BoneId],
                    source_topology: &[ScaleSourceTopologyRow],
                    transform_only_attachments: &[BoneId],
                    ledger: &ScaleLedger| {
        let reason = if affected_nodes != plan.affected_nodes() {
            Some("affected_nodes_mismatch")
        } else if source_topology != plan.source_topology {
            Some("affected_source_topology_mismatch")
        } else if transform_only_attachments != plan.transform_only_attachments() {
            Some("transform_only_attachments_mismatch")
        } else if ledger.obligations != plan.obligations() {
            Some("proof_obligations_mismatch")
        } else if ledger.payload_shapes != plan.ledger.payload_shapes {
            Some("payload_shape_inventory_mismatch")
        } else if ledger.field_rows != plan.field_rows() {
            Some("field_write_set_mismatch")
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(ScaleError::PlanDocumentMismatch { reason });
        }
        Ok(())
    };

    match plan.operation() {
        ScaleOperation::WholeDocumentLinearUnits { factor } => {
            let derived = plan_whole_document(document, factor)?;
            validate(
                derived.affected_nodes(),
                &derived.source_topology,
                derived.transform_only_attachments(),
                &derived.ledger,
            )?;
            Ok(())
        }
        ScaleOperation::RestBindUniformScale {
            source_skin_index,
            source_root_node_index,
            ..
        } => {
            let domain =
                derive_rest_bind_plan_domain(document, source_skin_index, source_root_node_index)?;
            let affected_nodes = domain.affected_nodes();
            let transform_only_attachments = domain.transform_only_attachments();
            let ledger = compile_scale_ledger(
                document,
                plan.operation(),
                &affected_nodes,
                &transform_only_attachments,
                &domain.source_rows,
            );
            validate(
                &affected_nodes,
                &domain.source_rows,
                &transform_only_attachments,
                &ledger,
            )?;
            Ok(())
        }
    }
}

pub(in crate::scale) fn classify_affine(
    linear: Mat3,
    tol: &ScaleTolerancePolicy,
) -> Result<f64, AffineDomainViolation> {
    classify_positive_uniform_affine(
        linear,
        PositiveUniformAffineTolerance {
            equal_axis: tol.equal_axis,
            relative_orthogonality: tol.relative_orthogonality,
            singular_determinant_relative: tol.singular_determinant_relative,
        },
    )
}

/// Which of the clip-driven obligations `document` carries evidence for
/// inside `affected`.
///
/// Each field is exactly the condition under which that obligation's residual
/// loop reads at least one payload, so a plan that declares the obligation is
/// declaring something [`super::prove_scale`] will actually check, and the residual
/// it reports is a measurement rather than the zero an empty loop leaves
/// behind. Computed the same way on both sides of the plan/proof boundary:
/// [`plan_scale`] uses it to decide what to declare, and [`super::prove_scale`] uses
/// it to fail closed when a declared obligation's evidence is not in the
/// document it was handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct SampledEvidence {
    /// Some clip declares a translation track on an affected bone. That
    /// track is both what puts key times into proof's `clip_sample_times` walk and the
    /// only payload its `check_track_value_residual` walk compares, so it is the
    /// whole evidence base for [`ScaleProofObligation::KeyTranslations`].
    key_translations: bool,
    /// Some clip declares *both* an affected cubic-spline track with at least
    /// two key times — which is what produces an interior time at all — and
    /// an affected translation track, which is what the comparison at that
    /// interior time reads. The two need not be the same track, but they must
    /// be in the same clip: interior times are harvested per clip and
    /// compared against that clip's tracks.
    cubic_interiors: bool,
    /// Some clip yields at least one sample time for an affected bone, of
    /// either kind. The trajectory obligation compares composed world
    /// matrices rather than track payloads, so any affected track's key times
    /// are evidence for it.
    sample_times: bool,
}

/// Measure [`SampledEvidence`] over `document`'s clips.
fn sampled_evidence(document: &Document, affected: &BTreeSet<BoneId>) -> SampledEvidence {
    let mut evidence = SampledEvidence::default();
    for clip in &document.clips {
        let mut translations = false;
        let mut cubic_segments = false;
        for track in &clip.tracks {
            if !affected.contains(&track.bone) || track.times.is_empty() {
                continue;
            }
            evidence.sample_times = true;
            translations |= track.property == Property::Translation;
            cubic_segments |=
                track.interpolation == Interpolation::CubicSpline && track.times.len() >= 2;
        }
        evidence.key_translations |= translations;
        evidence.cubic_interiors |= translations && cubic_segments;
    }
    evidence
}
