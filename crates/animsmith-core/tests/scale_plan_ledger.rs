use animsmith_core::glam::{Mat4, Quat, Vec3};
use animsmith_core::model::{
    MeshAsset, MeshInstance, Primitive, SceneAssets, SourceInverseBindAccessor, SourceSkinAsset,
    SourceSkinAttachment,
};
use animsmith_core::scale::{
    ScaleBoneRestField, ScaleCandidate, ScaleCapabilityCoverage, ScaleCapabilityFacts, ScaleError,
    ScaleFieldDisposition, ScaleFieldTarget, ScaleOperation, ScalePayloadShapeRow,
    ScaleProjectedRole, ScaleProofObligation, ScaleRequest, ScaleRewriteRule, ScaleSourceNodeKind,
    ScaleSourceRestField, build_scale_candidate, plan_scale, prove_scale,
};
use animsmith_core::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, SourceInverseBindAccessorStatus,
    SourceNodeAsset, SourceNodeLocalRest, SourceSkeletonAssets, SourceSkeletonCoverage, Track,
    TrackValues, Transform,
};

#[derive(Clone, Copy)]
struct RigNode {
    parent: Option<usize>,
    source_node_index: usize,
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}

fn rig(parent: Option<usize>, source_node_index: usize, translation: Vec3) -> RigNode {
    RigNode {
        parent,
        source_node_index,
        translation,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }
}

fn source_node(id: usize, node: RigNode, nodes: &[RigNode]) -> SourceNodeAsset {
    let mut source = SourceNodeAsset::new(
        node.source_node_index,
        SourceNodeLocalRest::Trs {
            translation: node.translation,
            rotation: node.rotation,
            scale: node.scale,
        },
    );
    source.name = Some(format!("bone{id}"));
    source.parent_source_node_index = node.parent.map(|parent| nodes[parent].source_node_index);
    source.scene_root_indices = node.parent.is_none().then_some(0).into_iter().collect();
    source.bone = Some(id);
    source
}

fn rig_document(nodes: &[RigNode], skin_bones: &[usize]) -> Document {
    let bones = nodes
        .iter()
        .enumerate()
        .map(|(id, node)| Bone {
            name: format!("bone{id}"),
            parent: node.parent,
            rest: Transform {
                translation: node.translation,
                rotation: node.rotation,
                scale: node.scale,
            },
            inverse_bind: None,
        })
        .collect();
    let source_nodes = nodes
        .iter()
        .copied()
        .enumerate()
        .map(|(id, node)| source_node(id, node, nodes))
        .collect();
    let joint_source_node_indices = skin_bones
        .iter()
        .map(|&bone| nodes[bone].source_node_index)
        .collect::<Vec<_>>();
    let mesh_owner = nodes[*skin_bones.last().expect("fixture has a joint")].source_node_index;

    let mut document = Document {
        skeleton: Skeleton { bones },
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "mesh".into(),
                source_mesh_index: 0,
                primitives: vec![Primitive {
                    positions: vec![Vec3::new(1.0, 0.0, 0.0)],
                    joints: vec![[0, 0, 0, 0]],
                    weights: vec![[1.0, 0.0, 0.0, 0.0]],
                    ..Primitive::default()
                }],
            }],
            instances: vec![MeshInstance {
                source_node_index: mesh_owner,
                node: skin_bones[0],
                mesh: 0,
                skin_joints: skin_bones.to_vec(),
                skin_ibms: vec![Mat4::IDENTITY; skin_bones.len()],
            }],
            source_skeleton: SourceSkeletonAssets {
                coverage: SourceSkeletonCoverage::Complete,
                nodes: source_nodes,
                skins: vec![SourceSkinAsset {
                    source_skin_index: 0,
                    name: None,
                    skeleton_root_source_node_index: None,
                    joint_source_node_indices,
                    inverse_bind_accessor: SourceInverseBindAccessor::default(),
                    attachments: Vec::new(),
                }],
            },
            ..SceneAssets::default()
        },
        ..Document::default()
    };

    let worlds = nodes
        .iter()
        .map(|node| {
            Mat4::from_scale_rotation_translation(node.scale, node.rotation, node.translation)
        })
        .collect::<Vec<_>>();
    let mut composed = Vec::with_capacity(nodes.len());
    for (node, local) in nodes.iter().zip(worlds) {
        composed.push(node.parent.map_or(local, |parent| composed[parent] * local));
    }
    document.assets.instances[0].skin_ibms = skin_bones
        .iter()
        .map(|&joint| composed[joint].inverse())
        .collect();
    document
}

fn unit_document() -> Document {
    rig_document(
        &[
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        ],
        &[1],
    )
}

fn compensated_document() -> Document {
    rig_document(
        &[
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.01),
            },
            RigNode {
                parent: Some(0),
                source_node_index: 1,
                translation: Vec3::new(0.0, 100.0, 0.0),
                rotation: Quat::from_rotation_y(0.2),
                scale: Vec3::ONE,
            },
            rig(Some(1), 2, Vec3::new(1.0, 0.0, 0.0)),
        ],
        &[1],
    )
}

fn compensated_document_with_connector() -> Document {
    let mut document = compensated_document();
    let connector_local = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
    document.skeleton.bones[1].rest.translation = Vec3::new(5.0, 100.0, 0.0);

    let child = document
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .expect("fixture projects the child");
    child.parent_source_node_index = Some(10);
    let mut connector = SourceNodeAsset::new(10, SourceNodeLocalRest::Matrix(connector_local));
    connector.parent_source_node_index = Some(0);
    document.assets.source_skeleton.nodes.push(connector);

    let root = document.skeleton.bones[0].rest.to_mat4();
    let child = document.skeleton.bones[1].rest.to_mat4();
    document.assets.instances[0].skin_ibms[0] = (root * child).inverse();
    document
}

fn add_unaffected_prop(document: &mut Document) -> usize {
    let bone = document.skeleton.bones.len();
    let source_node_index = 20;
    let rest = Transform {
        translation: Vec3::new(5.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    document.skeleton.bones.push(Bone {
        name: "prop".into(),
        parent: None,
        rest,
        inverse_bind: None,
    });
    let mut source = SourceNodeAsset::new(
        source_node_index,
        SourceNodeLocalRest::Trs {
            translation: rest.translation,
            rotation: rest.rotation,
            scale: rest.scale,
        },
    );
    source.scene_root_indices = vec![0];
    source.bone = Some(bone);
    document.assets.source_skeleton.nodes.push(source);
    document.assets.meshes.push(MeshAsset {
        name: "prop".into(),
        source_mesh_index: 1,
        primitives: vec![Primitive {
            positions: vec![Vec3::new(1.0, 0.0, 0.0)],
            ..Primitive::default()
        }],
    });
    document.assets.instances.push(MeshInstance {
        source_node_index,
        node: bone,
        mesh: 1,
        skin_joints: Vec::new(),
        skin_ibms: Vec::new(),
    });
    bone
}

fn add_unaffected_prop_leaf(document: &mut Document, parent: usize) -> usize {
    let bone = document.skeleton.bones.len();
    let source_node_index = 21;
    let rest = Transform {
        translation: Vec3::new(0.0, 2.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    document.skeleton.bones.push(Bone {
        name: "prop-leaf".into(),
        parent: Some(parent),
        rest,
        inverse_bind: None,
    });
    let mut source = SourceNodeAsset::new(
        source_node_index,
        SourceNodeLocalRest::Trs {
            translation: rest.translation,
            rotation: rest.rotation,
            scale: rest.scale,
        },
    );
    source.parent_source_node_index = Some(20);
    source.bone = Some(bone);
    document.assets.source_skeleton.nodes.push(source);
    bone
}

fn complete_capability() -> ScaleCapabilityFacts {
    let mut capability = ScaleCapabilityFacts::default();
    capability.coverage = ScaleCapabilityCoverage::Complete;
    capability
}

fn whole_document_plan(document: &Document, factor: f64) -> animsmith_core::scale::ScalePlan {
    let capability = complete_capability();
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        document,
        capability: &capability,
    })
    .expect("whole-document fixture plans")
}

fn rest_bind_plan(document: &Document) -> animsmith_core::scale::ScalePlan {
    rest_bind_plan_with_factor(document, 0.01)
}

fn rest_bind_plan_with_factor(
    document: &Document,
    expected_factor: f64,
) -> animsmith_core::scale::ScalePlan {
    let capability = complete_capability();
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor,
        },
        document,
        capability: &capability,
    })
    .expect("rest/bind fixture plans")
}

type TopologyKey = (usize, Option<usize>, ScaleSourceNodeKind);
type FieldKey = (ScaleFieldTarget, ScaleFieldDisposition, usize);
type RestMutation = (&'static str, fn(&mut Transform));

fn topology_keys(plan: &animsmith_core::scale::ScalePlan) -> Vec<TopologyKey> {
    plan.ledger()
        .source_topology()
        .map(|row| {
            (
                row.source_node_index(),
                row.parent_source_node_index(),
                row.kind(),
            )
        })
        .collect()
}

fn field_keys(plan: &animsmith_core::scale::ScalePlan) -> Vec<FieldKey> {
    plan.ledger()
        .field_rows()
        .map(|row| (row.target(), row.disposition(), row.element_count()))
        .collect()
}

fn payload_shapes(plan: &animsmith_core::scale::ScalePlan) -> Vec<ScalePayloadShapeRow> {
    plan.ledger().payload_shapes().copied().collect()
}

fn field(
    target: ScaleFieldTarget,
    disposition: ScaleFieldDisposition,
    element_count: usize,
) -> FieldKey {
    (target, disposition, element_count)
}

fn expected_rest_bind_fields(connector_tail: Option<usize>) -> Vec<FieldKey> {
    let preserve = ScaleFieldDisposition::PreserveExact;
    let parent_basis = ScaleFieldDisposition::Rewrite(ScaleRewriteRule::RestBindParentBasis);
    let local_scale = ScaleFieldDisposition::Rewrite(ScaleRewriteRule::RestBindLocalScale);
    let node_basis = ScaleFieldDisposition::Rewrite(ScaleRewriteRule::RestBindNodeBasis);
    let source_local = |connector_tail| {
        ScaleFieldDisposition::Rewrite(ScaleRewriteRule::RestBindSourceLocal { connector_tail })
    };
    let mut fields = vec![
        field(
            ScaleFieldTarget::BoneRest {
                bone: 0,
                field: ScaleBoneRestField::Translation,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::BoneRest {
                bone: 0,
                field: ScaleBoneRestField::Rotation,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::BoneRest {
                bone: 0,
                field: ScaleBoneRestField::Scale,
            },
            local_scale,
            1,
        ),
        field(
            ScaleFieldTarget::BoneRest {
                bone: 1,
                field: ScaleBoneRestField::Translation,
            },
            parent_basis,
            1,
        ),
        field(
            ScaleFieldTarget::BoneRest {
                bone: 1,
                field: ScaleBoneRestField::Rotation,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::BoneRest {
                bone: 1,
                field: ScaleBoneRestField::Scale,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::BoneRest {
                bone: 2,
                field: ScaleBoneRestField::Translation,
            },
            parent_basis,
            1,
        ),
        field(
            ScaleFieldTarget::BoneRest {
                bone: 2,
                field: ScaleBoneRestField::Rotation,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::BoneRest {
                bone: 2,
                field: ScaleBoneRestField::Scale,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::SourceNodeRest {
                source_node_index: 0,
                field: ScaleSourceRestField::Translation,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::SourceNodeRest {
                source_node_index: 0,
                field: ScaleSourceRestField::Rotation,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::SourceNodeRest {
                source_node_index: 0,
                field: ScaleSourceRestField::Scale,
            },
            source_local(None),
            1,
        ),
        field(
            ScaleFieldTarget::SourceNodeRest {
                source_node_index: 1,
                field: ScaleSourceRestField::Translation,
            },
            source_local(connector_tail),
            1,
        ),
        field(
            ScaleFieldTarget::SourceNodeRest {
                source_node_index: 1,
                field: ScaleSourceRestField::Rotation,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::SourceNodeRest {
                source_node_index: 1,
                field: ScaleSourceRestField::Scale,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::SourceNodeRest {
                source_node_index: 2,
                field: ScaleSourceRestField::Translation,
            },
            source_local(None),
            1,
        ),
        field(
            ScaleFieldTarget::SourceNodeRest {
                source_node_index: 2,
                field: ScaleSourceRestField::Rotation,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::SourceNodeRest {
                source_node_index: 2,
                field: ScaleSourceRestField::Scale,
            },
            preserve,
            1,
        ),
    ];
    if connector_tail.is_some() {
        fields.extend([
            field(
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index: 10,
                    field: ScaleSourceRestField::MatrixLinear,
                },
                preserve,
                1,
            ),
            field(
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index: 10,
                    field: ScaleSourceRestField::MatrixTranslation,
                },
                preserve,
                1,
            ),
            field(
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index: 10,
                    field: ScaleSourceRestField::MatrixHomogeneous,
                },
                preserve,
                1,
            ),
        ]);
    }
    fields.extend([
        field(
            ScaleFieldTarget::InstanceInverseBind {
                instance_index: 0,
                slot: 0,
                joint: 1,
            },
            node_basis,
            1,
        ),
        field(
            ScaleFieldTarget::MeshPositions {
                mesh_index: 0,
                primitive_index: 0,
            },
            preserve,
            1,
        ),
        field(
            ScaleFieldTarget::MeshNormals {
                mesh_index: 0,
                primitive_index: 0,
            },
            preserve,
            0,
        ),
    ]);
    fields
}

#[test]
fn whole_document_ledger_names_each_public_rewrite_and_obligation() {
    let mut document = unit_document();
    document.clips.push(Clip {
        name: "whole".into(),
        duration_s: 1.0,
        tracks: vec![
            Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::Y]),
            },
            Track {
                bone: 1,
                property: Property::Scale,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::ONE; 2]),
            },
        ],
    });
    let plan = whole_document_plan(&document, 0.01);
    let preserve = ScaleFieldDisposition::PreserveExact;
    let rewrite = ScaleFieldDisposition::Rewrite(ScaleRewriteRule::WholeDocumentLength);

    assert_eq!(
        topology_keys(&plan),
        vec![
            (
                0,
                None,
                ScaleSourceNodeKind::OutsideDomain { bone: Some(0) }
            ),
            (
                1,
                Some(0),
                ScaleSourceNodeKind::OutsideDomain { bone: Some(1) },
            ),
        ]
    );
    assert_eq!(
        payload_shapes(&plan),
        vec![
            ScalePayloadShapeRow::Document {
                bone_count: 2,
                source_node_count: 2,
                source_coverage: SourceSkeletonCoverage::Complete,
                clip_count: 1,
                instance_count: 1,
                mesh_count: 1,
            },
            ScalePayloadShapeRow::Bone {
                bone: 0,
                parent: None,
            },
            ScalePayloadShapeRow::Bone {
                bone: 1,
                parent: Some(0),
            },
            ScalePayloadShapeRow::SourceSkin {
                source_skin_index: 0,
                skeleton_root_source_node_index: None,
                joint_count: 1,
                attachment_count: 0,
                inverse_bind_status: SourceInverseBindAccessorStatus::Absent,
                inverse_bind_declared_count: None,
                inverse_bind_matrix_count: 0,
            },
            ScalePayloadShapeRow::SourceSkinJoint {
                source_skin_index: 0,
                slot: 0,
                source_node_index: 1,
            },
            ScalePayloadShapeRow::Clip {
                clip_index: 0,
                track_count: 2,
            },
            ScalePayloadShapeRow::Track {
                clip_index: 0,
                track_index: 0,
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                key_count: 2,
                value_count: 2,
            },
            ScalePayloadShapeRow::Track {
                clip_index: 0,
                track_index: 1,
                bone: 1,
                property: Property::Scale,
                interpolation: Interpolation::Linear,
                key_count: 2,
                value_count: 2,
            },
            ScalePayloadShapeRow::Instance {
                instance_index: 0,
                node: 1,
                source_node_index: 1,
                mesh: 0,
                joint_count: 1,
                inverse_bind_count: 1,
            },
            ScalePayloadShapeRow::InstanceJoint {
                instance_index: 0,
                slot: 0,
                joint: 1,
            },
            ScalePayloadShapeRow::Mesh {
                mesh_index: 0,
                source_mesh_index: 0,
                primitive_count: 1,
            },
            ScalePayloadShapeRow::Primitive {
                mesh_index: 0,
                primitive_index: 0,
                position_count: 1,
                normal_count: 0,
                joint_count: 1,
                weight_count: 1,
            },
        ]
    );
    assert_eq!(
        field_keys(&plan),
        vec![
            field(
                ScaleFieldTarget::BoneRest {
                    bone: 0,
                    field: ScaleBoneRestField::Translation,
                },
                rewrite,
                1,
            ),
            field(
                ScaleFieldTarget::BoneRest {
                    bone: 0,
                    field: ScaleBoneRestField::Rotation,
                },
                preserve,
                1,
            ),
            field(
                ScaleFieldTarget::BoneRest {
                    bone: 0,
                    field: ScaleBoneRestField::Scale,
                },
                preserve,
                1,
            ),
            field(
                ScaleFieldTarget::BoneRest {
                    bone: 1,
                    field: ScaleBoneRestField::Translation,
                },
                rewrite,
                1,
            ),
            field(
                ScaleFieldTarget::BoneRest {
                    bone: 1,
                    field: ScaleBoneRestField::Rotation,
                },
                preserve,
                1,
            ),
            field(
                ScaleFieldTarget::BoneRest {
                    bone: 1,
                    field: ScaleBoneRestField::Scale,
                },
                preserve,
                1,
            ),
            field(
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index: 0,
                    field: ScaleSourceRestField::Translation,
                },
                rewrite,
                1,
            ),
            field(
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index: 0,
                    field: ScaleSourceRestField::Rotation,
                },
                preserve,
                1,
            ),
            field(
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index: 0,
                    field: ScaleSourceRestField::Scale,
                },
                preserve,
                1,
            ),
            field(
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index: 1,
                    field: ScaleSourceRestField::Translation,
                },
                rewrite,
                1,
            ),
            field(
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index: 1,
                    field: ScaleSourceRestField::Rotation,
                },
                preserve,
                1,
            ),
            field(
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index: 1,
                    field: ScaleSourceRestField::Scale,
                },
                preserve,
                1,
            ),
            field(
                ScaleFieldTarget::AnimationValues {
                    clip_index: 0,
                    track_index: 0,
                    bone: 1,
                    property: Property::Translation,
                },
                rewrite,
                2,
            ),
            field(
                ScaleFieldTarget::AnimationValues {
                    clip_index: 0,
                    track_index: 1,
                    bone: 1,
                    property: Property::Scale,
                },
                preserve,
                2,
            ),
            field(
                ScaleFieldTarget::InstanceInverseBind {
                    instance_index: 0,
                    slot: 0,
                    joint: 1,
                },
                rewrite,
                1,
            ),
            field(
                ScaleFieldTarget::MeshPositions {
                    mesh_index: 0,
                    primitive_index: 0,
                },
                rewrite,
                1,
            ),
            field(
                ScaleFieldTarget::MeshNormals {
                    mesh_index: 0,
                    primitive_index: 0,
                },
                preserve,
                0,
            ),
        ]
    );
    assert_eq!(
        plan.ledger().obligations().copied().collect::<Vec<_>>(),
        vec![
            ScaleProofObligation::ExactTopology,
            ScaleProofObligation::ExactPayloadIdentity,
            ScaleProofObligation::RestWorld,
            ScaleProofObligation::TrackValues,
            ScaleProofObligation::MeshPositions,
            ScaleProofObligation::KeyTranslations,
            ScaleProofObligation::Trajectories,
            ScaleProofObligation::SkinAndBounds,
            ScaleProofObligation::AffectedInverseBinds,
        ]
    );

    let candidate = build_scale_candidate(&document, &plan).expect("ledger drives the build");
    prove_scale(&document, &candidate, &plan).expect("ledger obligations prove");
}

#[test]
fn factor_one_keeps_explicit_exact_field_claims() {
    let document = unit_document();
    let plan = whole_document_plan(&document, 1.0);

    assert!(plan.ledger().field_rows().next().is_some());
    assert!(
        plan.ledger()
            .field_rows()
            .all(|row| row.disposition() == ScaleFieldDisposition::PreserveExact),
        "a no-op plan must retain explicit inventory rows without analytic writes"
    );

    let candidate = build_scale_candidate(&document, &plan).expect("factor-one build succeeds");
    assert_eq!(
        candidate.document().skeleton.bones[1]
            .rest
            .translation
            .to_array()
            .map(f32::to_bits),
        document.skeleton.bones[1]
            .rest
            .translation
            .to_array()
            .map(f32::to_bits)
    );
    prove_scale(&document, &candidate, &plan).expect("factor-one obligations prove");
}

#[test]
fn normalized_candidate_values_remain_governed_by_versioned_residuals() {
    let document = unit_document();
    let plan = whole_document_plan(&document, 0.01);
    let candidate = build_scale_candidate(&document, &plan).expect("baseline candidate builds");
    let mut within_tolerance = candidate.document().clone();

    let translation = &mut within_tolerance.skeleton.bones[1].rest.translation.y;
    let expected_bits = translation.to_bits();
    *translation = f32::from_bits(expected_bits + 1);
    assert_ne!(translation.to_bits(), expected_bits);

    let proof = prove_scale(
        &document,
        &animsmith_core::scale::ScaleCandidate::from_document(within_tolerance),
        &plan,
    )
    .expect("one admitted normalized ulp remains a residual, not a structural mismatch");
    assert!(proof.rest_translation_residual > 0.0);
    assert!(proof.rest_translation_comparisons > 0);
}

#[test]
fn rest_bind_topology_distinguishes_direct_edges_and_connectors() {
    let direct = compensated_document();
    let direct_plan = rest_bind_plan(&direct);
    assert_eq!(
        topology_keys(&direct_plan),
        vec![
            (
                0,
                None,
                ScaleSourceNodeKind::Projected {
                    bone: 0,
                    role: ScaleProjectedRole::Root,
                    projected_parent: None,
                    incoming_connector_tail: None,
                },
            ),
            (
                1,
                Some(0),
                ScaleSourceNodeKind::Projected {
                    bone: 1,
                    role: ScaleProjectedRole::Joint,
                    projected_parent: Some(0),
                    incoming_connector_tail: None,
                },
            ),
            (
                2,
                Some(1),
                ScaleSourceNodeKind::Projected {
                    bone: 2,
                    role: ScaleProjectedRole::TransformOnly,
                    projected_parent: Some(1),
                    incoming_connector_tail: None,
                },
            ),
        ]
    );
    assert_eq!(field_keys(&direct_plan), expected_rest_bind_fields(None));
    assert_eq!(
        direct_plan
            .ledger()
            .obligations()
            .copied()
            .collect::<Vec<_>>(),
        vec![
            ScaleProofObligation::ExactTopology,
            ScaleProofObligation::ExactPayloadIdentity,
            ScaleProofObligation::RestWorldAndUnitScale,
            ScaleProofObligation::TransformOnlyAffine,
            ScaleProofObligation::MeshPositions,
            ScaleProofObligation::SkinAndBounds,
            ScaleProofObligation::AffectedInverseBinds,
        ]
    );

    let connected = compensated_document_with_connector();
    let connected_plan = rest_bind_plan(&connected);
    assert_eq!(
        topology_keys(&connected_plan),
        vec![
            (
                0,
                None,
                ScaleSourceNodeKind::Projected {
                    bone: 0,
                    role: ScaleProjectedRole::Root,
                    projected_parent: None,
                    incoming_connector_tail: None,
                },
            ),
            (
                1,
                Some(10),
                ScaleSourceNodeKind::Projected {
                    bone: 1,
                    role: ScaleProjectedRole::Joint,
                    projected_parent: Some(0),
                    incoming_connector_tail: Some(10),
                },
            ),
            (
                2,
                Some(1),
                ScaleSourceNodeKind::Projected {
                    bone: 2,
                    role: ScaleProjectedRole::TransformOnly,
                    projected_parent: Some(1),
                    incoming_connector_tail: None,
                },
            ),
            (10, Some(0), ScaleSourceNodeKind::Connector),
        ]
    );
    assert_eq!(
        field_keys(&connected_plan),
        expected_rest_bind_fields(Some(10))
    );
    assert_eq!(
        connected_plan
            .ledger()
            .obligations()
            .copied()
            .collect::<Vec<_>>(),
        vec![
            ScaleProofObligation::ExactTopology,
            ScaleProofObligation::ExactPayloadIdentity,
            ScaleProofObligation::RestWorldAndUnitScale,
            ScaleProofObligation::TransformOnlyAffine,
            ScaleProofObligation::MeshPositions,
            ScaleProofObligation::SkinAndBounds,
            ScaleProofObligation::AffectedInverseBinds,
            ScaleProofObligation::ExactConnectorProjection,
        ]
    );

    for (document, plan) in [(&direct, &direct_plan), (&connected, &connected_plan)] {
        let candidate = build_scale_candidate(document, plan).expect("rest/bind build succeeds");
        prove_scale(document, &candidate, plan).expect("rest/bind obligations prove");
    }
}

#[test]
fn obligations_are_derived_from_the_exact_evidence_inventory() {
    let mut document = compensated_document();
    document.clips.push(Clip {
        name: "translation".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO; 6]),
        }],
    });
    let plan = rest_bind_plan(&document);
    assert_eq!(
        plan.ledger().obligations().copied().collect::<Vec<_>>(),
        vec![
            ScaleProofObligation::ExactTopology,
            ScaleProofObligation::ExactPayloadIdentity,
            ScaleProofObligation::RestWorldAndUnitScale,
            ScaleProofObligation::TransformOnlyAffine,
            ScaleProofObligation::TrackValues,
            ScaleProofObligation::MeshPositions,
            ScaleProofObligation::KeyTranslations,
            ScaleProofObligation::CubicInteriors,
            ScaleProofObligation::Trajectories,
            ScaleProofObligation::SkinAndBounds,
            ScaleProofObligation::AffectedInverseBinds,
        ]
    );

    let mut bare = compensated_document();
    bare.assets.meshes.clear();
    bare.assets.instances.clear();
    bare.clips.clear();
    let bare_plan = rest_bind_plan(&bare);
    assert_eq!(
        bare_plan
            .ledger()
            .obligations()
            .copied()
            .collect::<Vec<_>>(),
        vec![
            ScaleProofObligation::ExactTopology,
            ScaleProofObligation::ExactPayloadIdentity,
            ScaleProofObligation::RestWorldAndUnitScale,
            ScaleProofObligation::TransformOnlyAffine,
        ]
    );
}

#[test]
fn unchanged_world_and_topology_are_explicit_and_enforced() {
    let mut document = compensated_document();
    let prop = add_unaffected_prop(&mut document);
    let leaf = add_unaffected_prop_leaf(&mut document, prop);
    let plan = rest_bind_plan(&document);

    assert_eq!(
        plan.ledger().obligations().copied().collect::<Vec<_>>(),
        vec![
            ScaleProofObligation::ExactTopology,
            ScaleProofObligation::ExactPayloadIdentity,
            ScaleProofObligation::ExactUnchangedWorldRest,
            ScaleProofObligation::RestWorldAndUnitScale,
            ScaleProofObligation::TransformOnlyAffine,
            ScaleProofObligation::MeshPositions,
            ScaleProofObligation::SkinAndBounds,
            ScaleProofObligation::AffectedInverseBinds,
        ]
    );

    let candidate = build_scale_candidate(&document, &plan).expect("baseline candidate builds");
    prove_scale(&document, &candidate, &plan).expect("baseline candidate proves");

    let mutations: [RestMutation; 3] = [
        ("translation", |rest| rest.translation.x += 1.0),
        ("rotation", |rest| {
            rest.rotation = Quat::from_rotation_x(0.25)
        }),
        ("scale", |rest| rest.scale.x = 2.0),
    ];
    for node in [prop, leaf] {
        for (name, mutate) in mutations {
            let mut changed = candidate.document().clone();
            mutate(&mut changed.skeleton.bones[node].rest);
            assert_eq!(
                prove_scale(
                    &document,
                    &animsmith_core::scale::ScaleCandidate::from_document(changed),
                    &plan,
                )
                .unwrap_err(),
                ScaleError::CandidateStructureMismatch {
                    reason: "unaffected_world_rest_mismatch",
                },
                "{name} mutation on unaffected bone {node}"
            );
        }
    }

    let mut reparented = candidate.document().clone();
    reparented.skeleton.bones[prop].parent = Some(2);
    reparented
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|row| row.bone == Some(prop))
        .expect("prop has a source row")
        .parent_source_node_index = Some(2);
    assert!(matches!(
        prove_scale(
            &document,
            &animsmith_core::scale::ScaleCandidate::from_document(reparented),
            &plan,
        ),
        Err(ScaleError::CandidateStructureMismatch { .. })
    ));
}

#[test]
fn numeric_only_replay_keeps_the_structural_ledger_valid() {
    let mut document = unit_document();
    document.clips.push(Clip {
        name: "translation".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::Y]),
        }],
    });
    let plan = whole_document_plan(&document, 0.01);

    let mut replay = document.clone();
    replay.clips[0].tracks[0].times[1] = 0.75;
    let TrackValues::Vec3s(values) = &mut replay.clips[0].tracks[0].values else {
        panic!("fixture translation track must carry vec3 values")
    };
    values[1] = Vec3::new(0.0, 3.0, 0.0);
    replay.assets.meshes[0].primitives[0].positions[0] = Vec3::new(4.0, 0.0, 0.0);

    let candidate =
        build_scale_candidate(&replay, &plan).expect("numeric-only replay still builds");
    prove_scale(&replay, &candidate, &plan).expect("numeric-only replay still proves");
}

#[test]
fn complete_source_node_backing_order_is_not_ledger_identity() {
    let document = unit_document();
    let plan = whole_document_plan(&document, 0.01);

    let mut replay = document.clone();
    replay.assets.source_skeleton.nodes.reverse();
    let candidate = build_scale_candidate(&replay, &plan)
        .expect("source-node backing order is not semantic identity");
    prove_scale(&replay, &candidate, &plan)
        .expect("canonical source identities remain valid after reorder");
}

#[test]
fn primitive_modeled_attribute_shape_is_exact_payload_identity() {
    let mut document = unit_document();
    document.assets.meshes[0].primitives[0].normals = vec![Vec3::Y];
    document.assets.meshes[0].primitives.push(Primitive {
        positions: vec![Vec3::X, Vec3::Y],
        normals: vec![Vec3::Z, -Vec3::Z],
        joints: vec![[0, 0, 0, 0]; 2],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; 2],
        ..Primitive::default()
    });
    let plan = whole_document_plan(&document, 0.01);
    assert_eq!(
        plan.ledger()
            .payload_shapes()
            .filter(|row| {
                matches!(
                    row,
                    ScalePayloadShapeRow::Mesh { .. } | ScalePayloadShapeRow::Primitive { .. }
                )
            })
            .copied()
            .collect::<Vec<_>>(),
        vec![
            ScalePayloadShapeRow::Mesh {
                mesh_index: 0,
                source_mesh_index: 0,
                primitive_count: 2,
            },
            ScalePayloadShapeRow::Primitive {
                mesh_index: 0,
                primitive_index: 0,
                position_count: 1,
                normal_count: 1,
                joint_count: 1,
                weight_count: 1,
            },
            ScalePayloadShapeRow::Primitive {
                mesh_index: 0,
                primitive_index: 1,
                position_count: 2,
                normal_count: 2,
                joint_count: 2,
                weight_count: 2,
            },
        ]
    );
    assert_eq!(
        field_keys(&plan)
            .into_iter()
            .filter(|(target, _, _)| matches!(target, ScaleFieldTarget::MeshNormals { .. }))
            .collect::<Vec<_>>(),
        vec![
            field(
                ScaleFieldTarget::MeshNormals {
                    mesh_index: 0,
                    primitive_index: 0,
                },
                ScaleFieldDisposition::PreserveExact,
                1,
            ),
            field(
                ScaleFieldTarget::MeshNormals {
                    mesh_index: 0,
                    primitive_index: 1,
                },
                ScaleFieldDisposition::PreserveExact,
                2,
            ),
        ]
    );
    let candidate = build_scale_candidate(&document, &plan).unwrap();

    let mut replay = document.clone();
    replay.assets.meshes[0].primitives[1].normals.clear();
    assert!(matches!(
        build_scale_candidate(&replay, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));
    assert!(matches!(
        prove_scale(&replay, &candidate, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));

    let mut malformed_candidate = candidate.document().clone();
    malformed_candidate.assets.meshes[0].primitives[1]
        .normals
        .clear();
    assert!(matches!(
        prove_scale(
            &document,
            &ScaleCandidate::from_document(malformed_candidate),
            &plan,
        ),
        Err(ScaleError::CandidateStructureMismatch {
            reason: "primitive_normal_count_mismatch"
        })
    ));

    let mut malformed_candidate = candidate.document().clone();
    malformed_candidate.assets.meshes[0].primitives[1]
        .joints
        .clear();
    assert!(matches!(
        prove_scale(
            &document,
            &ScaleCandidate::from_document(malformed_candidate),
            &plan,
        ),
        Err(ScaleError::CandidateStructureMismatch {
            reason: "primitive_joint_count_mismatch"
        })
    ));

    let mut malformed_candidate = candidate.into_document();
    malformed_candidate.assets.meshes[0].primitives[1]
        .weights
        .clear();
    assert!(matches!(
        prove_scale(
            &document,
            &ScaleCandidate::from_document(malformed_candidate),
            &plan,
        ),
        Err(ScaleError::CandidateStructureMismatch {
            reason: "primitive_weight_count_mismatch"
        })
    ));
}

#[test]
fn stable_source_mesh_identity_makes_same_shape_reordering_stale() {
    let mut document = Document::default();
    document.assets.meshes = vec![
        MeshAsset {
            name: "first".into(),
            source_mesh_index: 11,
            primitives: Vec::new(),
        },
        MeshAsset {
            name: "second".into(),
            source_mesh_index: 22,
            primitives: Vec::new(),
        },
    ];
    let plan = whole_document_plan(&document, 0.01);
    assert_eq!(
        plan.ledger()
            .payload_shapes()
            .filter(|row| matches!(row, ScalePayloadShapeRow::Mesh { .. }))
            .copied()
            .collect::<Vec<_>>(),
        vec![
            ScalePayloadShapeRow::Mesh {
                mesh_index: 0,
                source_mesh_index: 11,
                primitive_count: 0,
            },
            ScalePayloadShapeRow::Mesh {
                mesh_index: 1,
                source_mesh_index: 22,
                primitive_count: 0,
            },
        ]
    );
    let candidate = build_scale_candidate(&document, &plan).unwrap();

    let mut replay = document.clone();
    replay.assets.meshes.swap(0, 1);
    assert!(matches!(
        build_scale_candidate(&replay, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));
    assert!(matches!(
        prove_scale(&replay, &candidate, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));

    let mut malformed_candidate = candidate.into_document();
    malformed_candidate.assets.meshes.swap(0, 1);
    assert!(matches!(
        prove_scale(
            &document,
            &ScaleCandidate::from_document(malformed_candidate),
            &plan,
        ),
        Err(ScaleError::CandidateStructureMismatch {
            reason: "mesh_source_identity_mismatch"
        })
    ));
}

#[test]
fn factor_one_rest_bind_materializes_an_absent_format_defined_bind() {
    let mut document = unit_document();
    document.assets.instances[0].skin_ibms.clear();
    document.assets.source_skeleton.skins[0].attachments = vec![SourceSkinAttachment {
        source_node_index: document.assets.instances[0].source_node_index,
        source_mesh_index: Some(0),
    }];
    document.assets.source_skeleton.skins[0]
        .joint_source_node_indices
        .push(0);
    document.assets.source_skeleton.skins[0]
        .attachments
        .push(SourceSkinAttachment {
            source_node_index: 0,
            source_mesh_index: Some(0),
        });
    document.clips.push(Clip {
        name: "empty".into(),
        duration_s: 0.0,
        tracks: Vec::new(),
    });
    document.assets.meshes.push(MeshAsset {
        name: "empty".into(),
        source_mesh_index: 1,
        primitives: Vec::new(),
    });
    let plan = rest_bind_plan_with_factor(&document, 1.0);

    assert_eq!(
        payload_shapes(&plan),
        vec![
            ScalePayloadShapeRow::Document {
                bone_count: 2,
                source_node_count: 2,
                source_coverage: SourceSkeletonCoverage::Complete,
                clip_count: 1,
                instance_count: 1,
                mesh_count: 2,
            },
            ScalePayloadShapeRow::Bone {
                bone: 0,
                parent: None,
            },
            ScalePayloadShapeRow::Bone {
                bone: 1,
                parent: Some(0),
            },
            ScalePayloadShapeRow::SourceSkin {
                source_skin_index: 0,
                skeleton_root_source_node_index: None,
                joint_count: 2,
                attachment_count: 2,
                inverse_bind_status: SourceInverseBindAccessorStatus::Absent,
                inverse_bind_declared_count: None,
                inverse_bind_matrix_count: 0,
            },
            ScalePayloadShapeRow::SourceSkinJoint {
                source_skin_index: 0,
                slot: 0,
                source_node_index: 1,
            },
            ScalePayloadShapeRow::SourceSkinJoint {
                source_skin_index: 0,
                slot: 1,
                source_node_index: 0,
            },
            ScalePayloadShapeRow::SourceSkinAttachment {
                source_skin_index: 0,
                attachment_index: 0,
                source_node_index: 1,
                source_mesh_index: Some(0),
            },
            ScalePayloadShapeRow::SourceSkinAttachment {
                source_skin_index: 0,
                attachment_index: 1,
                source_node_index: 0,
                source_mesh_index: Some(0),
            },
            ScalePayloadShapeRow::Clip {
                clip_index: 0,
                track_count: 0,
            },
            ScalePayloadShapeRow::Instance {
                instance_index: 0,
                node: 1,
                source_node_index: 1,
                mesh: 0,
                joint_count: 1,
                inverse_bind_count: 0,
            },
            ScalePayloadShapeRow::InstanceJoint {
                instance_index: 0,
                slot: 0,
                joint: 1,
            },
            ScalePayloadShapeRow::Mesh {
                mesh_index: 0,
                source_mesh_index: 0,
                primitive_count: 1,
            },
            ScalePayloadShapeRow::Primitive {
                mesh_index: 0,
                primitive_index: 0,
                position_count: 1,
                normal_count: 0,
                joint_count: 1,
                weight_count: 1,
            },
            ScalePayloadShapeRow::Mesh {
                mesh_index: 1,
                source_mesh_index: 1,
                primitive_count: 0,
            },
        ]
    );

    let candidate = build_scale_candidate(&document, &plan)
        .expect("factor-one rest/bind still materializes bind evidence");
    assert!(document.assets.instances[0].skin_ibms.is_empty());
    assert_eq!(
        candidate.document().assets.instances[0].skin_ibms,
        vec![Mat4::IDENTITY]
    );
    prove_scale(&document, &candidate, &plan)
        .expect("the materialized factor-one bind remains provable");

    let mut mutations = Vec::new();
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0].source_skin_index = 7;
    mutations.push(("skin identity", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0].skeleton_root_source_node_index = Some(0);
    mutations.push(("skeleton root", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0]
        .joint_source_node_indices
        .swap(0, 1);
    mutations.push(("joint order", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0].attachments[0].source_node_index = 0;
    mutations.push(("attachment node", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0].attachments[0].source_mesh_index = None;
    mutations.push(("attachment mesh", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0].attachments.clear();
    mutations.push(("attachment count", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0]
        .attachments
        .swap(0, 1);
    mutations.push(("attachment order", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0]
        .joint_source_node_indices
        .push(1);
    mutations.push(("joint count", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0]
        .inverse_bind_accessor
        .status = SourceInverseBindAccessorStatus::Unreadable;
    mutations.push(("inverse-bind status", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0]
        .inverse_bind_accessor
        .declared_count = Some(1);
    mutations.push(("inverse-bind declared count", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins[0]
        .inverse_bind_accessor
        .matrices
        .push(Mat4::IDENTITY);
    mutations.push(("inverse-bind matrix count", external));
    let mut external = candidate.document().clone();
    external.assets.source_skeleton.skins.clear();
    mutations.push(("skin count", external));

    for (label, external) in mutations {
        assert!(
            matches!(
                prove_scale(&document, &ScaleCandidate::from_document(external), &plan),
                Err(ScaleError::CandidateStructureMismatch {
                    reason: "source_skin_payload_mismatch"
                })
            ),
            "{label}"
        );
    }
}

#[test]
fn unavailable_source_skins_do_not_become_ledger_identity() {
    let mut document = unit_document();
    document.assets.source_skeleton.coverage = SourceSkeletonCoverage::Unavailable;
    let plan = whole_document_plan(&document, 0.01);

    assert!(plan.ledger().payload_shapes().any(|row| matches!(
        row,
        ScalePayloadShapeRow::Document {
            bone_count: 2,
            source_node_count: 0,
            source_coverage: SourceSkeletonCoverage::Unavailable,
            clip_count: 0,
            instance_count: 1,
            mesh_count: 1,
        }
    )));
    assert!(!plan.ledger().payload_shapes().any(|row| matches!(
        row,
        ScalePayloadShapeRow::SourceSkin { .. }
            | ScalePayloadShapeRow::SourceSkinJoint { .. }
            | ScalePayloadShapeRow::SourceSkinAttachment { .. }
    )));

    let mut replay = document.clone();
    replay
        .assets
        .source_skeleton
        .nodes
        .push(SourceNodeAsset::new(
            99,
            SourceNodeLocalRest::Trs {
                translation: Vec3::new(3.0, 4.0, 5.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        ));
    replay.assets.source_skeleton.skins.clear();
    let candidate = build_scale_candidate(&replay, &plan).unwrap();
    prove_scale(&replay, &candidate, &plan).unwrap();
}

#[test]
fn candidate_source_skins_preserve_stable_source_order() {
    let mut document = unit_document();
    document.assets.source_skeleton.skins.push(SourceSkinAsset {
        source_skin_index: 7,
        name: Some("secondary".into()),
        skeleton_root_source_node_index: Some(0),
        joint_source_node_indices: Vec::new(),
        inverse_bind_accessor: SourceInverseBindAccessor::default(),
        attachments: Vec::new(),
    });
    let plan = whole_document_plan(&document, 0.01);
    assert!(plan.ledger().payload_shapes().any(|row| matches!(
        row,
        ScalePayloadShapeRow::SourceSkin {
            source_skin_index: 7,
            skeleton_root_source_node_index: Some(0),
            joint_count: 0,
            attachment_count: 0,
            inverse_bind_status: SourceInverseBindAccessorStatus::Absent,
            inverse_bind_declared_count: None,
            inverse_bind_matrix_count: 0,
        }
    )));
    let candidate = build_scale_candidate(&document, &plan).unwrap();

    let mut source_retargeted = document.clone();
    source_retargeted.assets.source_skeleton.skins[1].source_skin_index = 8;
    assert!(matches!(
        build_scale_candidate(&source_retargeted, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));
    assert!(matches!(
        prove_scale(&source_retargeted, &candidate, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));

    let mut source_reordered = document.clone();
    source_reordered.assets.source_skeleton.skins.swap(0, 1);
    assert!(matches!(
        build_scale_candidate(&source_reordered, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));
    assert!(matches!(
        prove_scale(&source_reordered, &candidate, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));

    let mut retargeted = candidate.document().clone();
    retargeted.assets.source_skeleton.skins[1].source_skin_index = 8;
    assert!(matches!(
        prove_scale(&document, &ScaleCandidate::from_document(retargeted), &plan),
        Err(ScaleError::CandidateStructureMismatch {
            reason: "source_skin_payload_mismatch"
        })
    ));

    let mut reordered = candidate.into_document();
    reordered.assets.source_skeleton.skins.swap(0, 1);
    assert!(matches!(
        prove_scale(&document, &ScaleCandidate::from_document(reordered), &plan),
        Err(ScaleError::CandidateStructureMismatch {
            reason: "source_skin_payload_mismatch"
        })
    ));
}

#[test]
fn replay_rejects_a_changed_inventory_with_the_same_old_boolean_summary() {
    let mut document = compensated_document();
    document.clips.push(Clip {
        name: "rotation-a".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![Quat::IDENTITY; 2]),
        }],
    });
    let plan = rest_bind_plan(&document);
    let candidate = build_scale_candidate(&document, &plan).expect("baseline source builds");

    let mut changed = document.clone();
    changed.clips.push(Clip {
        name: "rotation-b".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![Quat::IDENTITY; 2]),
        }],
    });

    assert!(matches!(
        build_scale_candidate(&changed, &plan),
        Err(ScaleError::PlanDocumentMismatch { .. })
    ));
    assert!(matches!(
        prove_scale(&changed, &candidate, &plan),
        Err(ScaleError::PlanDocumentMismatch { .. })
    ));
}

#[test]
fn replay_rejects_same_cardinality_source_attachment_retargets() {
    let mut document = unit_document();
    document.assets.source_skeleton.skins[0].attachments = vec![SourceSkinAttachment {
        source_node_index: document.assets.instances[0].source_node_index,
        source_mesh_index: Some(0),
    }];
    let plan = whole_document_plan(&document, 0.01);
    let candidate = build_scale_candidate(&document, &plan).expect("baseline source builds");

    let mut retargeted = document.clone();
    retargeted.assets.source_skeleton.skins[0].attachments[0].source_mesh_index = None;

    assert!(matches!(
        build_scale_candidate(&retargeted, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));
    assert!(matches!(
        prove_scale(&retargeted, &candidate, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));

    let mut retargeted = document.clone();
    retargeted.assets.source_skeleton.skins[0].attachments[0].source_node_index = 0;

    assert!(matches!(
        build_scale_candidate(&retargeted, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));
    assert!(matches!(
        prove_scale(&retargeted, &candidate, &plan),
        Err(ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        })
    ));
}
