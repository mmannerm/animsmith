//! Numeric-free projection of the core scale ledger onto raw glTF identity.
//!
//! This module owns the one structural binding pass over raw nodes, meshes,
//! skins, and animation channels. Writers and artifact proofs consume the
//! resulting identities and shapes, but independently derive every numeric
//! multiplier.

use super::GltfScaleRewriteError;
use super::proof::object;
use crate::LoadError;
use crate::capability::{GltfScaleSource, declared};
use animsmith_core::Property;
use animsmith_core::scale::{
    ScaleError, ScaleFieldDisposition, ScaleFieldTarget, ScaleOperation, ScalePayloadShapeRow,
    ScalePlan, ScaleRewriteRule, ScaleSourceNodeKind, ScaleSourceRestField,
};
use serde_json::{Map, Value};
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static RAW_SKIN_BIND_STEPS: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

#[cfg(test)]
fn record_raw_skin_attachment_lookup() {
    RAW_SKIN_BIND_STEPS.with(|steps| {
        if let Some((attachments, slots)) = steps.get() {
            steps.set(Some((attachments + 1, slots)));
        }
    });
}

#[cfg(test)]
fn record_raw_skin_instance_slot_check() {
    RAW_SKIN_BIND_STEPS.with(|steps| {
        if let Some((attachments, slots)) = steps.get() {
            steps.set(Some((attachments, slots + 1)));
        }
    });
}

/// Numeric-free component group for one raw accessor use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestBindComponents {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
    /// Column-major `MAT4` rows 0..2, preserving its homogeneous row.
    Mat4Rows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceRestKey {
    Translation,
    Rotation,
    Scale,
    MatrixLinear,
    MatrixTranslation,
    MatrixHomogeneous,
}

impl SourceRestKey {
    const COUNT: usize = 6;

    fn index(self) -> usize {
        match self {
            Self::Translation => 0,
            Self::Rotation => 1,
            Self::Scale => 2,
            Self::MatrixLinear => 3,
            Self::MatrixTranslation => 4,
            Self::MatrixHomogeneous => 5,
        }
    }
}

impl TryFrom<ScaleSourceRestField> for SourceRestKey {
    type Error = GltfScaleRewriteError;

    fn try_from(value: ScaleSourceRestField) -> Result<Self, Self::Error> {
        Ok(match value {
            ScaleSourceRestField::Translation => Self::Translation,
            ScaleSourceRestField::Rotation => Self::Rotation,
            ScaleSourceRestField::Scale => Self::Scale,
            ScaleSourceRestField::MatrixLinear => Self::MatrixLinear,
            ScaleSourceRestField::MatrixTranslation => Self::MatrixTranslation,
            ScaleSourceRestField::MatrixHomogeneous => Self::MatrixHomogeneous,
            _ => return Err(plan_mismatch("unsupported_source_rest_field")),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrackField {
    bone: usize,
    property: Property,
    disposition: ScaleFieldDisposition,
}

/// One raw node member inventory row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RawNodeBinding {
    pub(crate) source_node_index: usize,
    pub(crate) kind: ScaleSourceNodeKind,
    pub(crate) matrix_declared: bool,
    pub(crate) translation_declared: bool,
    fields: [Option<ScaleFieldDisposition>; SourceRestKey::COUNT],
}

/// One modeled instance inverse-bind slot attached to a raw skin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RawInstanceSlotBinding {
    pub(crate) joint: usize,
    pub(crate) disposition: ScaleFieldDisposition,
}

/// One source-skin joint and its disposition when a raw bind slot exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RawSkinSlotBinding {
    pub(crate) source_node_index: usize,
    pub(crate) disposition: Option<ScaleFieldDisposition>,
}

/// One source-order raw skin binding, retained even when no node uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawSkinBinding {
    pub(crate) source_skin_index: usize,
    pub(crate) slots: Vec<RawSkinSlotBinding>,
}

/// Structural meaning of one raw accessor use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RawAccessorTarget {
    PreserveExact,
    MeshPositions {
        disposition: ScaleFieldDisposition,
    },
    MeshNormals {
        disposition: ScaleFieldDisposition,
    },
    /// Raw-only glTF `POSITION` morph-target deltas. The normalized core has
    /// no morph model; this identity is admitted only by the whole-document
    /// raw writer.
    MorphPositions,
    InstanceInverseBind {
        source_skin_index: usize,
    },
    Animation {
        source_node_index: usize,
        property: Property,
        disposition: ScaleFieldDisposition,
    },
}

/// One raw accessor identity, its declared shape, and its numeric-free target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawAccessorBinding {
    pub(crate) accessor_index: usize,
    pub(crate) location: String,
    pub(crate) components: RestBindComponents,
    pub(crate) count: usize,
    pub(crate) target: RawAccessorTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeshPrimitiveFields {
    positions: ScaleFieldDisposition,
    normals: ScaleFieldDisposition,
}

#[derive(Debug, Clone)]
struct InstanceBinding {
    source_node_index: usize,
    slots: Vec<Option<RawInstanceSlotBinding>>,
}

/// One validated structural view of a [`ScalePlan`] in raw glTF identity.
pub(crate) struct GltfScalePlan {
    nodes: Vec<RawNodeBinding>,
    skins: Vec<RawSkinBinding>,
    accessor_bindings: Vec<RawAccessorBinding>,
}

impl GltfScalePlan {
    /// Replay the complete core inventory, cross-check raw hierarchy identity,
    /// then bind every raw accessor use exactly once.
    pub(crate) fn new(
        source: &GltfScaleSource,
        plan: &ScalePlan,
    ) -> Result<Self, GltfScaleRewriteError> {
        let operation = plan.operation();
        if matches!(operation, ScaleOperation::RestBindUniformScale { .. }) {
            check_projected_bones_are_unique(source)?;
        }
        plan.validate_document_inventory(source.document())
            .map_err(GltfScaleRewriteError::Plan)?;
        let root = object(source.raw_json())?;
        let topology = cross_check_raw_topology(root, plan)?;
        cross_check_source_skin_payload(source, root)?;

        let raw_nodes = array(root, "nodes");
        let mut source_fields = vec![[None; SourceRestKey::COUNT]; raw_nodes.len()];
        let mut track_fields = array(root, "animations")
            .iter()
            .map(|animation| vec![None; array_value(animation.get("channels")).len()])
            .collect::<Vec<_>>();

        let core_mesh_count = plan
            .ledger()
            .payload_shapes()
            .filter_map(|row| match row {
                ScalePayloadShapeRow::Mesh { mesh_index, .. } => Some(mesh_index + 1),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let mut source_mesh_of_core = vec![None; core_mesh_count];
        let mut primitive_count_of_core = vec![0; core_mesh_count];
        for row in plan.ledger().payload_shapes() {
            if let ScalePayloadShapeRow::Mesh {
                mesh_index,
                source_mesh_index,
                primitive_count,
            } = *row
            {
                set_once(
                    &mut source_mesh_of_core,
                    mesh_index,
                    source_mesh_index,
                    "duplicate_mesh_payload_identity",
                )?;
                primitive_count_of_core[mesh_index] = primitive_count;
            }
        }
        let mut core_mesh_fields = primitive_count_of_core
            .iter()
            .map(|&count| vec![(None, None); count])
            .collect::<Vec<_>>();
        let mut bone_inverse_binds = vec![None; source.document().skeleton.bones.len()];

        let instance_count = plan
            .ledger()
            .payload_shapes()
            .filter_map(|row| match row {
                ScalePayloadShapeRow::Instance { instance_index, .. } => Some(instance_index + 1),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let mut instances = vec![None; instance_count];
        for row in plan.ledger().payload_shapes() {
            if let ScalePayloadShapeRow::Instance {
                instance_index,
                source_node_index,
                joint_count,
                ..
            } = *row
            {
                set_once(
                    &mut instances,
                    instance_index,
                    InstanceBinding {
                        source_node_index,
                        slots: vec![None; joint_count],
                    },
                    "duplicate_instance_payload_identity",
                )?;
            }
        }

        for row in plan.ledger().field_rows() {
            match row.target() {
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index,
                    field,
                } => {
                    let fields = source_fields
                        .get_mut(source_node_index)
                        .ok_or_else(|| plan_mismatch("source_rest_node_out_of_range"))?;
                    set_slot_once(
                        &mut fields[SourceRestKey::try_from(field)?.index()],
                        row.disposition(),
                        "duplicate_source_rest_field",
                    )?;
                }
                ScaleFieldTarget::AnimationValues {
                    clip_index,
                    track_index,
                    bone,
                    property,
                } => {
                    let slot = track_fields
                        .get_mut(clip_index)
                        .and_then(|tracks| tracks.get_mut(track_index))
                        .ok_or_else(|| plan_mismatch("animation_field_out_of_range"))?;
                    set_slot_once(
                        slot,
                        TrackField {
                            bone,
                            property,
                            disposition: row.disposition(),
                        },
                        "duplicate_animation_field",
                    )?;
                }
                ScaleFieldTarget::MeshPositions {
                    mesh_index,
                    primitive_index,
                } => {
                    let slot = core_mesh_fields
                        .get_mut(mesh_index)
                        .and_then(|primitives| primitives.get_mut(primitive_index))
                        .ok_or_else(|| plan_mismatch("mesh_position_field_out_of_range"))?;
                    set_slot_once(
                        &mut slot.0,
                        row.disposition(),
                        "duplicate_mesh_position_field",
                    )?;
                }
                ScaleFieldTarget::MeshNormals {
                    mesh_index,
                    primitive_index,
                } => {
                    let slot = core_mesh_fields
                        .get_mut(mesh_index)
                        .and_then(|primitives| primitives.get_mut(primitive_index))
                        .ok_or_else(|| plan_mismatch("mesh_normal_field_out_of_range"))?;
                    set_slot_once(
                        &mut slot.1,
                        row.disposition(),
                        "duplicate_mesh_normal_field",
                    )?;
                }
                ScaleFieldTarget::InstanceInverseBind {
                    instance_index,
                    slot,
                    joint,
                } => {
                    let instance = instances
                        .get_mut(instance_index)
                        .and_then(Option::as_mut)
                        .ok_or_else(|| plan_mismatch("instance_payload_identity_missing"))?;
                    let target = instance
                        .slots
                        .get_mut(slot)
                        .ok_or_else(|| plan_mismatch("instance_inverse_bind_slot_out_of_range"))?;
                    set_slot_once(
                        target,
                        RawInstanceSlotBinding {
                            joint,
                            disposition: row.disposition(),
                        },
                        "duplicate_instance_inverse_bind_field",
                    )?;
                }
                ScaleFieldTarget::BoneInverseBind { bone } => {
                    set_once(
                        &mut bone_inverse_binds,
                        bone,
                        row.disposition(),
                        "duplicate_bone_inverse_bind_field",
                    )?;
                }
                _ => {}
            }
        }

        let raw_meshes = array(root, "meshes");
        let mut core_mesh_of_source = vec![None; raw_meshes.len()];
        for (core_mesh_index, &source_mesh_index) in source_mesh_of_core.iter().enumerate() {
            let source_mesh_index =
                source_mesh_index.ok_or_else(|| plan_mismatch("mesh_payload_identity_missing"))?;
            set_once(
                &mut core_mesh_of_source,
                source_mesh_index,
                core_mesh_index,
                "duplicate_source_mesh_payload_identity",
            )?;
        }
        let completed_core_mesh_fields = core_mesh_fields
            .into_iter()
            .map(|fields| {
                fields
                    .into_iter()
                    .map(|(positions, normals)| match (positions, normals) {
                        (Some(positions), Some(normals)) => {
                            Ok(MeshPrimitiveFields { positions, normals })
                        }
                        _ => Err(plan_mismatch("mesh_primitive_field_missing")),
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut mesh_fields = Vec::with_capacity(raw_meshes.len());
        for (source_mesh_index, mesh) in raw_meshes.iter().enumerate() {
            let raw_primitives = array_value(mesh.get("primitives"));
            let Some(core_mesh_index) = core_mesh_of_source[source_mesh_index] else {
                mesh_fields.push(vec![None; raw_primitives.len()]);
                continue;
            };
            let fields = completed_core_mesh_fields
                .get(core_mesh_index)
                .ok_or_else(|| plan_mismatch("mesh_payload_identity_missing"))?;
            mesh_fields.push(project_raw_primitives(root, raw_primitives, fields)?);
        }

        let nodes = topology
            .into_iter()
            .zip(source_fields)
            .enumerate()
            .map(|(source_node_index, (kind, fields))| {
                let raw = &raw_nodes[source_node_index];
                RawNodeBinding {
                    source_node_index,
                    kind,
                    matrix_declared: declared(raw, "matrix").is_some(),
                    translation_declared: declared(raw, "translation").is_some(),
                    fields,
                }
            })
            .collect::<Vec<_>>();

        let mut plan = Self {
            nodes,
            skins: Vec::new(),
            accessor_bindings: Vec::new(),
        };
        plan.skins = bind_raw_skins(source, &plan, &instances, &bone_inverse_binds)?;
        plan.accessor_bindings = bind_raw_accessors(root, &plan, &track_fields, &mesh_fields)?;
        validate_accessor_dispositions(&plan, operation)?;
        Ok(plan)
    }

    pub(crate) fn node_bindings(&self) -> &[RawNodeBinding] {
        &self.nodes
    }

    pub(crate) fn accessor_bindings(&self) -> &[RawAccessorBinding] {
        &self.accessor_bindings
    }

    pub(crate) fn skin_bindings(&self) -> &[RawSkinBinding] {
        &self.skins
    }

    pub(crate) fn skin_binding(
        &self,
        source_skin_index: usize,
    ) -> Result<&RawSkinBinding, GltfScaleRewriteError> {
        self.skins
            .get(source_skin_index)
            .filter(|skin| skin.source_skin_index == source_skin_index)
            .ok_or_else(|| plan_mismatch("raw_skin_binding_missing"))
    }

    pub(crate) fn source_rest(
        &self,
        source_node_index: usize,
        field: ScaleSourceRestField,
    ) -> Result<ScaleFieldDisposition, GltfScaleRewriteError> {
        let key = SourceRestKey::try_from(field)?;
        self.nodes
            .get(source_node_index)
            .and_then(|node| node.fields[key.index()])
            .ok_or_else(|| plan_mismatch("source_rest_field_missing"))
    }

    pub(crate) fn source_kind(&self, source_node_index: usize) -> Option<ScaleSourceNodeKind> {
        self.nodes.get(source_node_index).map(|node| node.kind)
    }

    pub(crate) fn projected_bone(&self, source_node_index: usize) -> Option<usize> {
        match self.source_kind(source_node_index)? {
            ScaleSourceNodeKind::Projected { bone, .. }
            | ScaleSourceNodeKind::OutsideDomain { bone: Some(bone) } => Some(bone),
            ScaleSourceNodeKind::Connector | ScaleSourceNodeKind::OutsideDomain { bone: None } => {
                None
            }
            _ => None,
        }
    }

    pub(crate) fn is_rest_bind_affected(&self, source_node_index: usize) -> bool {
        matches!(
            self.source_kind(source_node_index),
            Some(ScaleSourceNodeKind::Projected { .. } | ScaleSourceNodeKind::Connector)
        )
    }

    pub(crate) fn affected_source_nodes(&self, rest_bind: bool) -> Vec<usize> {
        self.nodes
            .iter()
            .filter_map(|node| {
                (!rest_bind
                    || matches!(
                        node.kind,
                        ScaleSourceNodeKind::Projected { .. } | ScaleSourceNodeKind::Connector
                    ))
                .then_some(node.source_node_index)
            })
            .collect()
    }
}

fn bind_raw_skins(
    source: &GltfScaleSource,
    plan: &GltfScalePlan,
    instances: &[Option<InstanceBinding>],
    bone_inverse_binds: &[Option<ScaleFieldDisposition>],
) -> Result<Vec<RawSkinBinding>, GltfScaleRewriteError> {
    let mut instances_by_source_node = vec![Vec::new(); plan.nodes.len()];
    for instance in instances.iter().flatten() {
        instances_by_source_node
            .get_mut(instance.source_node_index)
            .ok_or_else(|| plan_mismatch("instance_source_node_out_of_range"))?
            .push(instance);
    }

    source
        .document()
        .assets
        .source_skeleton
        .skins
        .iter()
        .enumerate()
        .map(|(source_skin_index, skin)| {
            if skin.source_skin_index != source_skin_index {
                return Err(plan_mismatch("source_skin_payload_identity_mismatch"));
            }
            let attached_instances = collect_attached_instances(
                skin.attachments
                    .iter()
                    .map(|attachment| attachment.source_node_index),
                &instances_by_source_node,
            )?;
            let has_inverse_binds = skin.inverse_bind_accessor.declared_count.is_some();
            let mut slots = Vec::with_capacity(skin.joint_source_node_indices.len());
            for (slot_index, &source_node_index) in
                skin.joint_source_node_indices.iter().enumerate()
            {
                let disposition = if has_inverse_binds {
                    let bone = plan
                        .projected_bone(source_node_index)
                        .ok_or_else(|| plan_mismatch("skin_joint_projection_missing"))?;
                    let mut attached_disposition = None;
                    for instance in &attached_instances {
                        #[cfg(test)]
                        record_raw_skin_instance_slot_check();
                        let field = instance
                            .slots
                            .get(slot_index)
                            .and_then(|field| *field)
                            .ok_or_else(|| plan_mismatch("instance_inverse_bind_field_missing"))?;
                        if field.joint != bone {
                            return Err(plan_mismatch("instance_inverse_bind_joint_mismatch"));
                        }
                        match attached_disposition {
                            Some(previous) if previous != field.disposition => {
                                return Err(plan_mismatch(
                                    "instance_inverse_bind_disposition_conflict",
                                ));
                            }
                            Some(_) => {}
                            None => attached_disposition = Some(field.disposition),
                        }
                    }
                    Some(match attached_disposition {
                        Some(disposition) => disposition,
                        None => bone_inverse_binds
                            .get(bone)
                            .and_then(|field| *field)
                            .ok_or_else(|| plan_mismatch("bone_inverse_bind_field_missing"))?,
                    })
                } else {
                    None
                };
                slots.push(RawSkinSlotBinding {
                    source_node_index,
                    disposition,
                });
            }
            Ok(RawSkinBinding {
                source_skin_index,
                slots,
            })
        })
        .collect()
}

fn collect_attached_instances<'a>(
    source_nodes: impl IntoIterator<Item = usize>,
    instances_by_source_node: &'a [Vec<&'a InstanceBinding>],
) -> Result<Vec<&'a InstanceBinding>, GltfScaleRewriteError> {
    let mut attached = Vec::new();
    for source_node_index in source_nodes {
        #[cfg(test)]
        record_raw_skin_attachment_lookup();
        attached.extend(
            instances_by_source_node
                .get(source_node_index)
                .ok_or_else(|| plan_mismatch("skin_attachment_source_node_out_of_range"))?,
        );
    }
    Ok(attached)
}

fn raw_primitive_is_modeled(
    root: &Map<String, Value>,
    primitive: &Value,
) -> Result<bool, GltfScaleRewriteError> {
    let mode = primitive.get("mode").and_then(Value::as_u64).unwrap_or(4);
    if mode != 4 {
        return Ok(false);
    }
    let Some(position) = primitive
        .get("attributes")
        .and_then(Value::as_object)
        .and_then(|attributes| attributes.get("POSITION"))
    else {
        return Ok(false);
    };
    let accessor_index =
        as_index(Some(position)).ok_or_else(|| plan_mismatch("mesh_position_accessor_invalid"))?;
    let accessor = array(root, "accessors")
        .get(accessor_index)
        .ok_or_else(|| plan_mismatch("mesh_position_accessor_out_of_range"))?;
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| plan_mismatch("mesh_position_count_invalid"))?;
    Ok(count > 0)
}

fn project_raw_primitives(
    root: &Map<String, Value>,
    raw_primitives: &[Value],
    compact_fields: &[MeshPrimitiveFields],
) -> Result<Vec<Option<MeshPrimitiveFields>>, GltfScaleRewriteError> {
    let mut next_core_primitive = 0;
    let mut projected = Vec::with_capacity(raw_primitives.len());
    for primitive in raw_primitives {
        if raw_primitive_is_modeled(root, primitive)? {
            let field = compact_fields
                .get(next_core_primitive)
                .copied()
                .ok_or_else(|| plan_mismatch("mesh_primitive_projection_overflow"))?;
            projected.push(Some(field));
            next_core_primitive += 1;
        } else {
            projected.push(None);
        }
    }
    if next_core_primitive != compact_fields.len() {
        return Err(plan_mismatch("mesh_primitive_projection_count_mismatch"));
    }
    Ok(projected)
}

fn validate_accessor_dispositions(
    plan: &GltfScalePlan,
    operation: ScaleOperation,
) -> Result<(), GltfScaleRewriteError> {
    let (whole_document, factor_changes) = match operation {
        ScaleOperation::WholeDocumentLinearUnits { factor } => (true, factor != 1.0),
        ScaleOperation::RestBindUniformScale {
            expected_factor, ..
        } => (false, expected_factor != 1.0),
        _ => return Err(plan_mismatch("unsupported_scale_operation")),
    };
    let expected_length = || {
        if factor_changes {
            ScaleFieldDisposition::Rewrite(ScaleRewriteRule::WholeDocumentLength)
        } else {
            ScaleFieldDisposition::PreserveExact
        }
    };
    for binding in &plan.accessor_bindings {
        match &binding.target {
            RawAccessorTarget::PreserveExact => {}
            RawAccessorTarget::MeshNormals { disposition } => {
                require_disposition(
                    *disposition,
                    ScaleFieldDisposition::PreserveExact,
                    "invalid_mesh_normal_disposition",
                )?;
            }
            RawAccessorTarget::MeshPositions { disposition } => {
                let expected = if whole_document {
                    expected_length()
                } else {
                    ScaleFieldDisposition::PreserveExact
                };
                require_disposition(*disposition, expected, "invalid_mesh_position_disposition")?;
            }
            RawAccessorTarget::MorphPositions => {
                if !whole_document {
                    return Err(plan_mismatch("morph_positions_unsupported_for_rest_bind"));
                }
            }
            RawAccessorTarget::Animation {
                property,
                disposition,
                ..
            } => match (whole_document, property) {
                (true, Property::Translation) => require_disposition(
                    *disposition,
                    expected_length(),
                    "invalid_animation_translation_disposition",
                )?,
                (true, _) => require_disposition(
                    *disposition,
                    ScaleFieldDisposition::PreserveExact,
                    "invalid_animation_preserved_disposition",
                )?,
                (false, Property::Translation) => require_allowed_rest_disposition(
                    *disposition,
                    ScaleRewriteRule::RestBindParentBasis,
                    "invalid_animation_translation_disposition",
                )?,
                (false, Property::Scale) => require_allowed_rest_disposition(
                    *disposition,
                    ScaleRewriteRule::RestBindLocalScale,
                    "invalid_animation_scale_disposition",
                )?,
                (false, _) => require_disposition(
                    *disposition,
                    ScaleFieldDisposition::PreserveExact,
                    "invalid_animation_preserved_disposition",
                )?,
            },
            RawAccessorTarget::InstanceInverseBind { source_skin_index } => {
                let skin = plan.skin_binding(*source_skin_index)?;
                for slot in &skin.slots {
                    let disposition = slot
                        .disposition
                        .ok_or_else(|| plan_mismatch("inverse_bind_disposition_missing"))?;
                    if whole_document {
                        require_disposition(
                            disposition,
                            expected_length(),
                            "invalid_inverse_bind_disposition",
                        )?;
                    } else {
                        require_allowed_rest_disposition(
                            disposition,
                            ScaleRewriteRule::RestBindNodeBasis,
                            "invalid_inverse_bind_disposition",
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn require_disposition(
    actual: ScaleFieldDisposition,
    expected: ScaleFieldDisposition,
    reason: &'static str,
) -> Result<(), GltfScaleRewriteError> {
    if actual != expected {
        return Err(plan_mismatch(reason));
    }
    Ok(())
}

fn require_allowed_rest_disposition(
    disposition: ScaleFieldDisposition,
    rewrite: ScaleRewriteRule,
    reason: &'static str,
) -> Result<(), GltfScaleRewriteError> {
    let allowed = match disposition {
        ScaleFieldDisposition::PreserveExact => true,
        ScaleFieldDisposition::Rewrite(rule) => rule == rewrite,
        _ => false,
    };
    if !allowed {
        return Err(plan_mismatch(reason));
    }
    Ok(())
}

fn animation_field(
    track_fields: &[Vec<Option<TrackField>>],
    plan: &GltfScalePlan,
    animation_index: usize,
    channel_index: usize,
    target_source_node_index: usize,
    property: Property,
) -> Result<ScaleFieldDisposition, GltfScaleRewriteError> {
    let field = track_fields
        .get(animation_index)
        .and_then(|tracks| tracks.get(channel_index))
        .and_then(|field| *field)
        .ok_or_else(|| plan_mismatch("animation_field_missing"))?;
    let target_bone = plan
        .projected_bone(target_source_node_index)
        .ok_or_else(|| plan_mismatch("animation_target_projection_missing"))?;
    if field.bone != target_bone || field.property != property {
        return Err(plan_mismatch("animation_field_identity_mismatch"));
    }
    Ok(field.disposition)
}

fn mesh_field(
    mesh_fields: &[Vec<Option<MeshPrimitiveFields>>],
    source_mesh_index: usize,
    primitive_index: usize,
) -> Result<Option<MeshPrimitiveFields>, GltfScaleRewriteError> {
    mesh_fields
        .get(source_mesh_index)
        .and_then(|primitives| primitives.get(primitive_index))
        .copied()
        .ok_or_else(|| plan_mismatch("mesh_primitive_field_missing"))
}

fn bind_raw_accessors(
    root: &Map<String, Value>,
    plan: &GltfScalePlan,
    track_fields: &[Vec<Option<TrackField>>],
    mesh_fields: &[Vec<Option<MeshPrimitiveFields>>],
) -> Result<Vec<RawAccessorBinding>, GltfScaleRewriteError> {
    let mut bindings = Vec::new();
    let mut bind = |accessor_index: usize,
                    location: String,
                    required: Option<RestBindComponents>,
                    target: RawAccessorTarget|
     -> Result<usize, GltfScaleRewriteError> {
        let (components, count) = accessor_shape(root, accessor_index)?;
        if required.is_some_and(|required| accessor_type(required) != accessor_type(components)) {
            return Err(unrewritable(accessor_index));
        }
        bindings.push(RawAccessorBinding {
            accessor_index,
            location,
            components: required.unwrap_or(components),
            count,
            target,
        });
        Ok(count)
    };

    for (mesh_index, mesh) in array(root, "meshes").iter().enumerate() {
        for (primitive_index, primitive) in array_value(mesh.get("primitives")).iter().enumerate() {
            let base = format!("/meshes/{mesh_index}/primitives/{primitive_index}");
            let mut base_position_count = None;
            if let Some(attributes) = primitive.get("attributes").and_then(Value::as_object) {
                for (semantic, value) in attributes {
                    let Some(accessor_index) = as_index(Some(value)) else {
                        continue;
                    };
                    let fields = mesh_field(mesh_fields, mesh_index, primitive_index)?;
                    let (required, target) = match semantic.as_str() {
                        "POSITION" => match fields {
                            Some(fields) => (
                                Some(RestBindComponents::Vec3),
                                RawAccessorTarget::MeshPositions {
                                    disposition: fields.positions,
                                },
                            ),
                            None => (None, RawAccessorTarget::PreserveExact),
                        },
                        "NORMAL" => match fields {
                            Some(fields) => (
                                Some(RestBindComponents::Vec3),
                                RawAccessorTarget::MeshNormals {
                                    disposition: fields.normals,
                                },
                            ),
                            None => (None, RawAccessorTarget::PreserveExact),
                        },
                        _ => (None, RawAccessorTarget::PreserveExact),
                    };
                    let count = bind(
                        accessor_index,
                        format!("{base}/attributes/{semantic}"),
                        required,
                        target,
                    )?;
                    if semantic == "POSITION" {
                        base_position_count = Some(count);
                    }
                }
            }
            if let Some(accessor_index) = as_index(primitive.get("indices")) {
                bind(
                    accessor_index,
                    format!("{base}/indices"),
                    None,
                    RawAccessorTarget::PreserveExact,
                )?;
            }
            for (target_index, target) in array_value(primitive.get("targets")).iter().enumerate() {
                let Some(target) = target.as_object() else {
                    continue;
                };
                for (semantic, value) in target {
                    if let Some(accessor_index) = as_index(Some(value)) {
                        let (required, target) = if semantic == "POSITION" {
                            (
                                Some(RestBindComponents::Vec3),
                                RawAccessorTarget::MorphPositions,
                            )
                        } else {
                            (None, RawAccessorTarget::PreserveExact)
                        };
                        let count = bind(
                            accessor_index,
                            format!("{base}/targets/{target_index}/{semantic}"),
                            required,
                            target,
                        )?;
                        if semantic == "POSITION"
                            && base_position_count.is_none_or(|base_count| base_count != count)
                        {
                            return Err(plan_mismatch("morph_position_count_mismatch"));
                        }
                    }
                }
            }
        }
    }

    for (skin_index, skin) in array(root, "skins").iter().enumerate() {
        let Some(accessor_index) = as_index(skin.get("inverseBindMatrices")) else {
            continue;
        };
        let skin_binding = plan.skin_binding(skin_index)?;
        let count = bind(
            accessor_index,
            format!("/skins/{skin_index}/inverseBindMatrices"),
            Some(RestBindComponents::Mat4Rows),
            RawAccessorTarget::InstanceInverseBind {
                source_skin_index: skin_index,
            },
        )?;
        if skin_binding.slots.len() != count {
            return Err(unrewritable(accessor_index));
        }
    }

    for (animation_index, animation) in array(root, "animations").iter().enumerate() {
        let samplers = array_value(animation.get("samplers"));
        let mut referenced = vec![false; samplers.len()];
        for channel in array_value(animation.get("channels")) {
            if let Some(sampler) = as_index(channel.get("sampler"))
                && let Some(slot) = referenced.get_mut(sampler)
            {
                *slot = true;
            }
        }
        for (sampler_index, sampler) in samplers.iter().enumerate() {
            if let Some(accessor_index) = as_index(sampler.get("input")) {
                bind(
                    accessor_index,
                    format!("/animations/{animation_index}/samplers/{sampler_index}/input"),
                    None,
                    RawAccessorTarget::PreserveExact,
                )?;
            }
            if !referenced[sampler_index]
                && let Some(accessor_index) = as_index(sampler.get("output"))
            {
                bind(
                    accessor_index,
                    format!("/animations/{animation_index}/samplers/{sampler_index}/output"),
                    None,
                    RawAccessorTarget::PreserveExact,
                )?;
            }
        }
        for (channel_index, channel) in array_value(animation.get("channels")).iter().enumerate() {
            let target = channel.get("target");
            let Some(path) = target
                .and_then(|target| target.get("path"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            let Some(accessor_index) = as_index(channel.get("sampler"))
                .and_then(|sampler| samplers.get(sampler))
                .and_then(|sampler| as_index(sampler.get("output")))
            else {
                continue;
            };
            let (required, target) = match path {
                "translation" | "scale" | "rotation" => {
                    let source_node_index = as_index(target.and_then(|target| target.get("node")))
                        .ok_or_else(|| plan_mismatch("raw_animation_target_missing"))?;
                    let property = match path {
                        "translation" => Property::Translation,
                        "scale" => Property::Scale,
                        "rotation" => Property::Rotation,
                        _ => unreachable!(),
                    };
                    (
                        matches!(property, Property::Translation | Property::Scale)
                            .then_some(RestBindComponents::Vec3),
                        RawAccessorTarget::Animation {
                            source_node_index,
                            property,
                            disposition: animation_field(
                                track_fields,
                                plan,
                                animation_index,
                                channel_index,
                                source_node_index,
                                property,
                            )?,
                        },
                    )
                }
                "weights" => (None, RawAccessorTarget::PreserveExact),
                _ => continue,
            };
            bind(
                accessor_index,
                format!("/animations/{animation_index}/channels/{channel_index}"),
                required,
                target,
            )?;
        }
    }
    Ok(bindings)
}

fn accessor_shape(
    root: &Map<String, Value>,
    accessor_index: usize,
) -> Result<(RestBindComponents, usize), GltfScaleRewriteError> {
    let accessor = array(root, "accessors")
        .get(accessor_index)
        .ok_or_else(|| unrewritable(accessor_index))?;
    let components = match accessor.get("type").and_then(Value::as_str) {
        Some("SCALAR") => RestBindComponents::Scalar,
        Some("VEC2") => RestBindComponents::Vec2,
        Some("VEC3") => RestBindComponents::Vec3,
        Some("VEC4") => RestBindComponents::Vec4,
        Some("MAT2") => RestBindComponents::Mat2,
        Some("MAT3") => RestBindComponents::Mat3,
        Some("MAT4") => RestBindComponents::Mat4,
        _ => return Err(unrewritable(accessor_index)),
    };
    accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or_else(|| unrewritable(accessor_index))?;
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| unrewritable(accessor_index))?;
    Ok((components, count))
}

pub(crate) fn accessor_type(components: RestBindComponents) -> &'static str {
    match components {
        RestBindComponents::Scalar => "SCALAR",
        RestBindComponents::Vec2 => "VEC2",
        RestBindComponents::Vec3 => "VEC3",
        RestBindComponents::Vec4 => "VEC4",
        RestBindComponents::Mat2 => "MAT2",
        RestBindComponents::Mat3 => "MAT3",
        RestBindComponents::Mat4 | RestBindComponents::Mat4Rows => "MAT4",
    }
}

fn unrewritable(accessor_index: usize) -> GltfScaleRewriteError {
    GltfScaleRewriteError::UnrewritableAccessor {
        accessor_index,
        location: format!("/accessors/{accessor_index}"),
    }
}

fn set_once<T>(
    values: &mut [Option<T>],
    index: usize,
    value: T,
    reason: &'static str,
) -> Result<(), GltfScaleRewriteError> {
    let slot = values.get_mut(index).ok_or_else(|| plan_mismatch(reason))?;
    set_slot_once(slot, value, reason)
}

fn set_slot_once<T>(
    slot: &mut Option<T>,
    value: T,
    reason: &'static str,
) -> Result<(), GltfScaleRewriteError> {
    if slot.replace(value).is_some() {
        return Err(plan_mismatch(reason));
    }
    Ok(())
}

pub(super) fn cross_check_source_skin_payload(
    source: &GltfScaleSource,
    root: &Map<String, Value>,
) -> Result<(), GltfScaleRewriteError> {
    let raw_skins = array(root, "skins");
    if raw_skins.len() != source.document().assets.source_skeleton.skins.len() {
        return Err(plan_mismatch("raw_source_skin_count_mismatch"));
    }
    for (skin_index, (raw, modeled)) in raw_skins
        .iter()
        .zip(&source.document().assets.source_skeleton.skins)
        .enumerate()
    {
        if modeled.source_skin_index != skin_index
            || as_index(raw.get("skeleton")) != modeled.skeleton_root_source_node_index
        {
            return Err(plan_mismatch("raw_source_skin_identity_mismatch"));
        }
        let raw_joints = array_value(raw.get("joints"))
            .iter()
            .map(|joint| {
                as_index(Some(joint)).ok_or_else(|| plan_mismatch("raw_source_skin_joint_invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if raw_joints != modeled.joint_source_node_indices {
            return Err(plan_mismatch("raw_source_skin_joint_mismatch"));
        }
    }
    Ok(())
}

fn cross_check_raw_topology(
    root: &Map<String, Value>,
    plan: &ScalePlan,
) -> Result<Vec<ScaleSourceNodeKind>, GltfScaleRewriteError> {
    let nodes = array(root, "nodes");
    let mut raw_parents = vec![None; nodes.len()];
    let mut raw_children = vec![Vec::new(); nodes.len()];
    for (parent, node) in nodes.iter().enumerate() {
        for child in array_value(node.get("children")) {
            let child = as_index(Some(child)).ok_or_else(|| {
                LoadError::Malformed(format!("/nodes/{parent}/children entry is not an index"))
            })?;
            let slot = raw_parents.get_mut(child).ok_or(
                GltfScaleRewriteError::UnusableSourceHierarchy {
                    reason: "a child index is not a source node",
                },
            )?;
            if slot.replace(parent).is_some() {
                return Err(GltfScaleRewriteError::UnusableSourceHierarchy {
                    reason: "a node is named as a child by two parents",
                });
            }
            raw_children[parent].push(child);
        }
    }

    let mut topology = vec![None; nodes.len()];
    for row in plan.ledger().source_topology() {
        let source = row.source_node_index();
        set_once(
            &mut topology,
            source,
            row.kind(),
            "duplicate_source_topology_identity",
        )?;
    }
    if let ScaleOperation::RestBindUniformScale {
        source_skin_index,
        source_root_node_index,
        ..
    } = plan.operation()
    {
        let derived = raw_rest_bind_closure(
            root,
            &raw_parents,
            &raw_children,
            source_skin_index,
            source_root_node_index,
        )?;
        let planned = topology
            .iter()
            .enumerate()
            .filter_map(|(source, kind)| {
                matches!(
                    kind,
                    Some(ScaleSourceNodeKind::Projected { .. } | ScaleSourceNodeKind::Connector)
                )
                .then_some(source)
            })
            .collect::<Vec<_>>();
        if planned != derived {
            return Err(GltfScaleRewriteError::ClosureMismatch { planned, derived });
        }
    }
    for row in plan.ledger().source_topology() {
        let source = row.source_node_index();
        if raw_parents.get(source).copied().flatten() != row.parent_source_node_index() {
            return Err(GltfScaleRewriteError::ParentChainDisagreement {
                source_node_index: source,
            });
        }
    }
    topology
        .into_iter()
        .map(|kind| kind.ok_or_else(|| plan_mismatch("raw_source_topology_count_mismatch")))
        .collect()
}

fn check_projected_bones_are_unique(source: &GltfScaleSource) -> Result<(), GltfScaleRewriteError> {
    let mut source_of_bone = vec![None; source.document().skeleton.bones.len()];
    for node in &source.document().assets.source_skeleton.nodes {
        let Some(bone) = node.bone else { continue };
        let slot = source_of_bone
            .get_mut(bone)
            .ok_or_else(|| plan_mismatch("source_projection_bone_out_of_range"))?;
        if slot.replace(node.source_node_index).is_some() {
            return Err(GltfScaleRewriteError::AmbiguousSourceNodeProjection { bone });
        }
    }
    Ok(())
}

fn raw_rest_bind_closure(
    root: &Map<String, Value>,
    parents: &[Option<usize>],
    children: &[Vec<usize>],
    source_skin_index: usize,
    source_root_node_index: usize,
) -> Result<Vec<usize>, GltfScaleRewriteError> {
    if source_root_node_index >= parents.len() {
        return Err(GltfScaleRewriteError::UnusableSourceHierarchy {
            reason: "the selected root node index is not a source node",
        });
    }
    let skin = array(root, "skins").get(source_skin_index).ok_or(
        GltfScaleRewriteError::UnusableSourceHierarchy {
            reason: "the selected skin index is not a source skin",
        },
    )?;
    let mut closure = vec![false; parents.len()];
    closure[source_root_node_index] = true;
    let mut reaches_root = closure.clone();
    let mut visit_generation = vec![0usize; parents.len()];
    let mut generation = 0usize;
    for joint in array_value(skin.get("joints")) {
        let mut cursor = as_index(Some(joint)).ok_or_else(|| {
            LoadError::Malformed(format!(
                "/skins/{source_skin_index}/joints entry is not an index"
            ))
        })?;
        if cursor >= parents.len() {
            return Err(GltfScaleRewriteError::UnusableSourceHierarchy {
                reason: "a skin joint index is not a source node",
            });
        }
        generation = generation.wrapping_add(1);
        if generation == 0 {
            visit_generation.fill(0);
            generation = 1;
        }
        let mut pending = Vec::new();
        while !reaches_root[cursor] {
            if visit_generation[cursor] == generation {
                return Err(GltfScaleRewriteError::UnusableSourceHierarchy {
                    reason: "a skin joint's parent chain is cyclic or unbounded",
                });
            }
            visit_generation[cursor] = generation;
            pending.push(cursor);
            let Some(parent) = parents[cursor] else {
                return Err(GltfScaleRewriteError::UnusableSourceHierarchy {
                    reason: "a skin joint is not a descendant of the selected root node",
                });
            };
            cursor = parent;
        }
        for source in pending {
            reaches_root[source] = true;
            closure[source] = true;
        }
    }
    let mut queue = closure
        .iter()
        .enumerate()
        .filter_map(|(source, &inside)| inside.then_some(source))
        .collect::<Vec<_>>();
    while let Some(source) = queue.pop() {
        for &child in &children[source] {
            if !closure[child] {
                closure[child] = true;
                queue.push(child);
            }
        }
    }
    Ok(closure
        .into_iter()
        .enumerate()
        .filter_map(|(source, inside)| inside.then_some(source))
        .collect())
}

fn array<'a>(root: &'a Map<String, Value>, key: &str) -> &'a [Value] {
    array_value(root.get(key))
}

fn array_value(value: Option<&Value>) -> &[Value] {
    value.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn as_index(value: Option<&Value>) -> Option<usize> {
    value?
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
}

pub(crate) fn plan_mismatch(reason: &'static str) -> GltfScaleRewriteError {
    GltfScaleRewriteError::Plan(ScaleError::PlanDocumentMismatch { reason })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn skipped_raw_primitive_keeps_the_following_compacted_primitive_aligned() {
        let root = json!({
            "accessors": [
                { "count": 3 },
                { "count": 3 }
            ]
        });
        let raw = json!([
            { "attributes": { "NORMAL": 0 } },
            { "attributes": { "POSITION": 1 } }
        ]);
        let field = MeshPrimitiveFields {
            positions: ScaleFieldDisposition::Rewrite(ScaleRewriteRule::WholeDocumentLength),
            normals: ScaleFieldDisposition::PreserveExact,
        };
        let projected = project_raw_primitives(
            root.as_object().expect("root object"),
            raw.as_array().expect("primitive array"),
            &[field],
        )
        .expect("loader-compatible projection");
        assert_eq!(projected, vec![None, Some(field)]);
    }

    #[test]
    fn rest_bind_closure_rejects_malformed_selector_and_parent_boundaries() {
        fn reason(error: GltfScaleRewriteError) -> &'static str {
            match error {
                GltfScaleRewriteError::UnusableSourceHierarchy { reason } => reason,
                other => panic!("unexpected error: {other:?}"),
            }
        }

        let root = json!({ "skins": [{ "joints": [1] }] });
        let root = root.as_object().expect("root object");
        assert_eq!(
            reason(raw_rest_bind_closure(root, &[None], &[vec![]], 0, 1).unwrap_err()),
            "the selected root node index is not a source node"
        );
        assert_eq!(
            reason(
                raw_rest_bind_closure(root, &[None, Some(0)], &[vec![1], vec![]], 1, 0)
                    .unwrap_err()
            ),
            "the selected skin index is not a source skin"
        );

        let out_of_range_joint = json!({ "skins": [{ "joints": [2] }] });
        assert_eq!(
            reason(
                raw_rest_bind_closure(
                    out_of_range_joint.as_object().expect("root object"),
                    &[None, Some(0)],
                    &[vec![1], vec![]],
                    0,
                    0,
                )
                .unwrap_err()
            ),
            "a skin joint index is not a source node"
        );

        let cyclic = json!({ "skins": [{ "joints": [1] }] });
        assert_eq!(
            reason(
                raw_rest_bind_closure(
                    cyclic.as_object().expect("root object"),
                    &[None, Some(2), Some(1)],
                    &[vec![], vec![2], vec![1]],
                    0,
                    0,
                )
                .unwrap_err()
            ),
            "a skin joint's parent chain is cyclic or unbounded"
        );

        let outside_root = json!({ "skins": [{ "joints": [2] }] });
        assert_eq!(
            reason(
                raw_rest_bind_closure(
                    outside_root.as_object().expect("root object"),
                    &[None, Some(0), None],
                    &[vec![1], vec![], vec![]],
                    0,
                    0,
                )
                .unwrap_err()
            ),
            "a skin joint is not a descendant of the selected root node"
        );
    }

    #[test]
    fn raw_accessor_binding_rechecks_required_type_and_inverse_bind_count() {
        let mismatched_mesh = json!({
            "accessors": [{ "componentType": 5126, "count": 1, "type": "VEC4" }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]
        });
        let empty_plan = GltfScalePlan {
            nodes: Vec::new(),
            skins: Vec::new(),
            accessor_bindings: Vec::new(),
        };
        let fields = MeshPrimitiveFields {
            positions: ScaleFieldDisposition::Rewrite(ScaleRewriteRule::WholeDocumentLength),
            normals: ScaleFieldDisposition::PreserveExact,
        };
        assert!(matches!(
            bind_raw_accessors(
                mismatched_mesh.as_object().expect("root object"),
                &empty_plan,
                &[],
                &[vec![Some(fields)]],
            ),
            Err(GltfScaleRewriteError::UnrewritableAccessor {
                accessor_index: 0,
                ..
            })
        ));

        let mismatched_bind_count = json!({
            "accessors": [{ "componentType": 5126, "count": 2, "type": "MAT4" }],
            "skins": [{ "joints": [0], "inverseBindMatrices": 0 }]
        });
        let skin_plan = GltfScalePlan {
            nodes: Vec::new(),
            skins: vec![RawSkinBinding {
                source_skin_index: 0,
                slots: vec![RawSkinSlotBinding {
                    source_node_index: 0,
                    disposition: Some(ScaleFieldDisposition::PreserveExact),
                }],
            }],
            accessor_bindings: Vec::new(),
        };
        assert!(matches!(
            bind_raw_accessors(
                mismatched_bind_count.as_object().expect("root object"),
                &skin_plan,
                &[],
                &[],
            ),
            Err(GltfScaleRewriteError::UnrewritableAccessor {
                accessor_index: 0,
                ..
            })
        ));
    }

    #[test]
    fn skin_only_attachments_are_classified_once_before_joint_slot_work() {
        const JOINTS: usize = 32;
        const ATTACHMENTS: usize = 128;

        let identity = [
            1.0f32, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];
        let mut buffer = Vec::with_capacity(JOINTS * 64);
        for _ in 0..JOINTS {
            for value in identity {
                buffer.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut nodes = vec![json!({}); JOINTS];
        nodes.extend((0..ATTACHMENTS).map(|_| json!({ "skin": 0 })));
        let value = json!({
            "asset": { "version": "2.0" },
            "buffers": [{
                "uri": format!(
                    "data:application/octet-stream;base64,{}",
                    STANDARD.encode(&buffer)
                ),
                "byteLength": buffer.len()
            }],
            "bufferViews": [{ "buffer": 0, "byteLength": buffer.len() }],
            "accessors": [{
                "bufferView": 0,
                "componentType": 5126,
                "count": JOINTS,
                "type": "MAT4"
            }],
            "nodes": nodes,
            "skins": [{
                "joints": (0..JOINTS).collect::<Vec<_>>(),
                "inverseBindMatrices": 0
            }]
        });
        let bytes = serde_json::to_vec(&value).expect("fixture serializes");
        let source =
            crate::preflight_scale_source_bytes(Path::new("many-empty-skin-nodes.gltf"), &bytes)
                .expect("skin-only attachment nodes are accepted");
        assert_eq!(
            source.document().assets.source_skeleton.skins[0]
                .attachments
                .len(),
            ATTACHMENTS
        );
        let capability = super::super::capability_facts(source.manifest());
        let plan = animsmith_core::scale::plan_scale(&animsmith_core::scale::ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
            document: source.document(),
            capability: &capability,
        })
        .expect("the hostile structural fixture plans");

        RAW_SKIN_BIND_STEPS.with(|steps| steps.set(Some((0, 0))));
        GltfScalePlan::new(&source, &plan).expect("the canonical adapter binds the fixture");
        let (attachment_lookups, instance_slot_checks) = RAW_SKIN_BIND_STEPS
            .with(|steps| steps.replace(None))
            .expect("test step counting was enabled");
        assert_eq!(
            attachment_lookups, ATTACHMENTS,
            "each attachment bucket is classified once, not once per joint"
        );
        assert_eq!(
            instance_slot_checks, 0,
            "skin-only nodes create no modeled instance-slot work"
        );
    }
}
