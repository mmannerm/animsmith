use super::*;
use crate::model::{
    Bone, MeshAsset, MeshInstance, Primitive, SceneAssets, SourceInverseBindAccessor,
    SourceNodeAsset, SourceNodeLocalRest, SourceProjectionViolation, SourceSkeletonAssets,
    SourceSkeletonCoverage, SourceSkinAsset, SourceSkinAttachment, Track,
};
use glam::{Quat, Vec3};

/// One node in a test rig, in ascending [`BoneId`] order (`nodes[i].bone
/// == i`): building both the normalized [`Skeleton`] and the format-neutral
/// `source_skeleton` from one list keeps them consistent by construction.
struct RigNode {
    parent: Option<BoneId>,
    source_node_index: usize,
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}

fn rig(parent: Option<BoneId>, source_node_index: usize, translation: Vec3) -> RigNode {
    RigNode {
        parent,
        source_node_index,
        translation,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }
}

/// Build a `Document` with a skinned rig from `nodes`, plus a matching
/// `assets.source_skeleton` projection (possibly using different source
/// node numbering than bone-id order) and one skin selecting `skin_bones`.
fn rig_document(
    nodes: &[RigNode],
    skin_bones: &[BoneId],
    skin_source_index: usize,
    ibm: Mat4,
) -> Document {
    let bones: Vec<Bone> = nodes
        .iter()
        .enumerate()
        .map(|(id, n)| Bone {
            name: format!("bone{id}"),
            parent: n.parent,
            rest: Transform {
                translation: n.translation,
                rotation: n.rotation,
                scale: n.scale,
            },
            inverse_bind: None,
        })
        .collect();
    let source_nodes: Vec<SourceNodeAsset> = nodes
        .iter()
        .enumerate()
        .map(|(id, n)| SourceNodeAsset {
            source_node_index: n.source_node_index,
            name: None,
            parent_source_node_index: n.parent.map(|p| nodes[p].source_node_index),
            scene_root_indices: if n.parent.is_none() { vec![0] } else { vec![] },
            local_rest: SourceNodeLocalRest::Trs {
                translation: n.translation,
                rotation: n.rotation,
                scale: n.scale,
            },
            bone: Some(id),
        })
        .collect();
    let joint_source_node_indices: Vec<usize> = skin_bones
        .iter()
        .map(|&b| nodes[b].source_node_index)
        .collect();
    let mesh_owner_source_index =
        nodes[*skin_bones.last().expect("at least one joint")].source_node_index;

    Document {
        skeleton: Skeleton { bones },
        clips: Vec::new(),
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
                source_node_index: mesh_owner_source_index,
                node: skin_bones[0],
                mesh: 0,
                skin_joints: skin_bones.to_vec(),
                skin_ibms: vec![ibm; skin_bones.len()],
            }],
            source_skeleton: SourceSkeletonAssets {
                coverage: SourceSkeletonCoverage::Complete,
                nodes: source_nodes,
                skins: vec![SourceSkinAsset {
                    source_skin_index: skin_source_index,
                    name: None,
                    skeleton_root_source_node_index: None,
                    joint_source_node_indices,
                    inverse_bind_accessor: SourceInverseBindAccessor::default(),
                    attachments: Vec::new(),
                }],
            },
            ..SceneAssets::default()
        },
        source: Default::default(),
    }
}

fn complete_capability() -> ScaleCapabilityFacts {
    ScaleCapabilityFacts {
        coverage: ScaleCapabilityCoverage::Complete,
        ..Default::default()
    }
}

fn unit_rig() -> Vec<RigNode> {
    vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
    ]
}

#[test]
fn assembly_basis_fingerprints_target_factors_and_rejects_orientation_or_helper_drift() {
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
    ];
    let mut document = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    document.clips.push(Clip {
        name: "cubic".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO; 6]),
        }],
    });
    let capability = complete_capability();
    let operation = ScaleOperation::RestBindUniformScale {
        source_skin_index: 0,
        source_root_node_index: 0,
        expected_factor: 0.01,
    };
    let plan = plan_scale(&ScaleRequest {
        operation,
        document: &document,
        capability: &capability,
    })
    .unwrap();
    let basis = assembly_scale_basis(&document, &plan).unwrap();
    assert_eq!(basis.version, ASSEMBLY_SCALE_BASIS_VERSION);
    assert_eq!(
        basis.tolerance_policy_id,
        ScaleTolerancePolicy::APPENDIX_D_V6.id
    );
    assert_eq!(basis.target_paths.len(), 1);
    assert_eq!(basis.target_paths[0].factor_bits, 0.01f64.to_bits());

    let named_operation = ScaleOperation::RestBindUniformScale {
        source_skin_index: 0,
        source_root_node_index: 1,
        expected_factor: 0.01,
    };
    let named_plan = plan_scale(&ScaleRequest {
        operation: named_operation,
        document: &document,
        capability: &capability,
    })
    .unwrap();
    let named = assembly_scale_compatibility_basis(
        &document,
        &named_plan,
        AssemblyScaleSelectorRequest::Named {
            root_node_name: "bone1",
        },
    )
    .unwrap();
    require_assembly_scale_compatibility_with_selectors(&named, &named).unwrap();
    assert!(
        assembly_scale_compatibility_basis(
            &document,
            &plan,
            AssemblyScaleSelectorRequest::Named {
                root_node_name: "bone1",
            },
        )
        .unwrap_err()
        .to_string()
        .contains("assembly_basis_named_selector_root_disagrees_with_plan")
    );

    let mut orientation = document.clone();
    orientation.skeleton.bones[1].rest.rotation = Quat::from_rotation_z(0.01);
    if let SourceNodeLocalRest::Trs { rotation, .. } =
        &mut orientation.assets.source_skeleton.nodes[1].local_rest
    {
        *rotation = Quat::from_rotation_z(0.01);
    }
    let orientation_plan = plan_scale(&ScaleRequest {
        operation,
        document: &orientation,
        capability: &capability,
    })
    .unwrap();
    let orientation_basis = assembly_scale_basis(&orientation, &orientation_plan).unwrap();
    assert_eq!(
        require_assembly_scale_compatibility(&basis, &orientation_basis)
            .unwrap_err()
            .reason,
        "named-orientation"
    );

    let mut equivalent = basis.clone();
    equivalent.named_nodes[1].rotation_bits = equivalent.named_nodes[1]
        .rotation_bits
        .map(|bits| (-f32::from_bits(bits)).to_bits());
    equivalent.named_nodes[1].translation_bits[1] =
        f32::from_bits(equivalent.named_nodes[1].translation_bits[1])
            .next_up()
            .to_bits();
    let AssemblyScaleSourceRest::Trs {
        translation_bits,
        rotation_bits,
        ..
    } = &mut equivalent.source_nodes[1].local_rest
    else {
        panic!("fixture source node uses TRS")
    };
    *rotation_bits = rotation_bits.map(|bits| (-f32::from_bits(bits)).to_bits());
    translation_bits[1] = f32::from_bits(translation_bits[1]).next_up().to_bits();
    assert_eq!(basis, basis.clone());
    assert_ne!(basis, equivalent, "fingerprint material remains exact");
    require_assembly_scale_compatibility(&basis, &equivalent)
        .expect("q/-q and in-band numeric spelling are semantically equivalent");

    for (changed, expected) in [
        (
            {
                let mut changed = basis.clone();
                changed.version += 1;
                changed
            },
            "basis-version",
        ),
        (
            {
                let mut changed = basis.clone();
                changed.coordinate_convention = "left-handed-z-up-centimetres";
                changed
            },
            "coordinate-convention",
        ),
        (
            {
                let mut changed = basis.clone();
                changed.tolerance_policy_id = "appendix-d-v999";
                changed
            },
            "tolerance-policy",
        ),
        (
            {
                let mut changed = basis.clone();
                changed.source_skin_index += 1;
                changed
            },
            "source-skin-selector",
        ),
        (
            {
                let mut changed = basis.clone();
                changed.source_root_node_index += 1;
                changed
            },
            "source-root-selector",
        ),
        (
            {
                let mut changed = basis.clone();
                changed.expected_factor_bits = 0.02f64.to_bits();
                changed
            },
            "expected-factor",
        ),
        (
            {
                let mut changed = basis.clone();
                changed.named_nodes[1].parent = None;
                changed
            },
            "named-topology",
        ),
        (
            {
                let mut changed = basis.clone();
                changed.named_nodes[1].translation_bits[1] = 101.0f32.to_bits();
                changed
            },
            "named-rest-basis",
        ),
    ] {
        assert_eq!(
            require_assembly_scale_compatibility(&basis, &changed)
                .unwrap_err()
                .reason,
            expected
        );
    }
    let mut distinct_take = basis.clone();
    distinct_take.target_paths[0].bone = "another-valid-take-target".into();
    distinct_take.target_paths[0].factor_bits = 0.5f64.to_bits();
    assert_ne!(
        basis, distinct_take,
        "target paths remain fingerprint material"
    );
    require_assembly_scale_compatibility(&basis, &distinct_take)
        .expect("each input validates its own target paths and plan factors");

    let mut helper = document.clone();
    helper.assets.source_skeleton.nodes[1].name = Some("changed-helper-name".into());
    let helper_plan = plan_scale(&ScaleRequest {
        operation,
        document: &helper,
        capability: &capability,
    })
    .unwrap();
    let helper_basis = assembly_scale_basis(&helper, &helper_plan).unwrap();
    assert_eq!(
        require_assembly_scale_compatibility(&basis, &helper_basis)
            .unwrap_err()
            .reason,
        "source-helper-layout"
    );

    let mut connector = document.clone();
    connector.assets.source_skeleton.nodes[1].parent_source_node_index = Some(2);
    let mut unnamed = SourceNodeAsset::new(2, SourceNodeLocalRest::Matrix(Mat4::IDENTITY));
    unnamed.parent_source_node_index = Some(0);
    connector.assets.source_skeleton.nodes.push(unnamed);
    let connector_plan = plan_scale(&ScaleRequest {
        operation,
        document: &connector,
        capability: &capability,
    })
    .unwrap();
    let connector_basis = assembly_scale_basis(&connector, &connector_plan).unwrap();
    assert_eq!(
        require_assembly_scale_compatibility(&basis, &connector_basis)
            .unwrap_err()
            .reason,
        "source-helper-layout"
    );
    let mut connector_matrix = connector.clone();
    connector_matrix.assets.source_skeleton.nodes[2].local_rest =
        SourceNodeLocalRest::Matrix(Mat4::from_translation(Vec3::new(0.1, 0.0, 0.0)));
    let connector_matrix_plan = plan_scale(&ScaleRequest {
        operation,
        document: &connector_matrix,
        capability: &capability,
    })
    .unwrap();
    let connector_matrix_basis =
        assembly_scale_basis(&connector_matrix, &connector_matrix_plan).unwrap();
    assert_eq!(
        require_assembly_scale_compatibility(&connector_basis, &connector_matrix_basis)
            .unwrap_err()
            .reason,
        "source-helper-rest-basis"
    );
}

#[test]
fn named_assembly_compatibility_rejects_a_different_resolved_skin_joint_identity() {
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
        rig(Some(1), 2, Vec3::new(0.0, 100.0, 0.0)),
    ];
    let base_document = rig_document(&nodes, &[1], 3, Mat4::IDENTITY);
    let input_document = rig_document(&nodes, &[1, 2], 7, Mat4::IDENTITY);
    let capability = complete_capability();
    let base_plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 3,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
        document: &base_document,
        capability: &capability,
    })
    .unwrap();
    let input_plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 7,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
        document: &input_document,
        capability: &capability,
    })
    .unwrap();
    let selector = AssemblyScaleSelectorRequest::Named {
        root_node_name: "bone1",
    };
    let base = assembly_scale_compatibility_basis(&base_document, &base_plan, selector).unwrap();
    let input = assembly_scale_compatibility_basis(&input_document, &input_plan, selector).unwrap();

    assert_ne!(
        base.basis().source_skin_index,
        input.basis().source_skin_index
    );
    assert_eq!(
        require_assembly_scale_compatibility_with_selectors(&base, &input)
            .unwrap_err()
            .reason,
        "source-name-selector"
    );
}

#[test]
fn named_assembly_compatibility_constructor_rejects_a_plan_for_another_skin() {
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
        rig(Some(1), 2, Vec3::new(0.0, 100.0, 0.0)),
    ];
    let mut document = rig_document(&nodes, &[1], 3, Mat4::IDENTITY);
    document.assets.source_skeleton.skins.push(SourceSkinAsset {
        source_skin_index: 7,
        name: None,
        skeleton_root_source_node_index: None,
        joint_source_node_indices: vec![2],
        inverse_bind_accessor: SourceInverseBindAccessor::default(),
        attachments: Vec::new(),
    });
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 7,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
        document: &document,
        capability: &capability,
    })
    .unwrap();

    assert!(
        assembly_scale_compatibility_basis(
            &document,
            &plan,
            AssemblyScaleSelectorRequest::Named {
                root_node_name: "bone1",
            },
        )
        .unwrap_err()
        .to_string()
        .contains("assembly_basis_named_selector_skin_disagrees_with_plan")
    );
}

#[test]
fn named_assembly_compatibility_checks_helper_layout_rest_and_semantic_paths() {
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
    ];
    let document = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let compatibility = |document: &Document, source_skin_index, source_root_node_index| {
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index,
                source_root_node_index,
                expected_factor: 0.01,
            },
            document,
            capability: &capability,
        })
        .unwrap();
        assembly_scale_compatibility_basis(
            document,
            &plan,
            AssemblyScaleSelectorRequest::Named {
                root_node_name: "bone1",
            },
        )
        .unwrap()
    };
    let base = compatibility(&document, 0, 1);

    let mut changed_name = document.clone();
    changed_name.assets.source_skeleton.nodes[1].name = Some("changed-helper-name".into());
    let changed_name = compatibility(&changed_name, 0, 1);
    assert_eq!(
        require_assembly_scale_compatibility_with_selectors(&base, &changed_name)
            .unwrap_err()
            .reason,
        "source-helper-layout"
    );

    let mut connector = document.clone();
    connector.assets.source_skeleton.nodes[1].parent_source_node_index = Some(2);
    let mut unnamed = SourceNodeAsset::new(2, SourceNodeLocalRest::Matrix(Mat4::IDENTITY));
    unnamed.parent_source_node_index = Some(0);
    connector.assets.source_skeleton.nodes.push(unnamed);
    let connector_basis = compatibility(&connector, 0, 1);
    assert_eq!(
        require_assembly_scale_compatibility_with_selectors(&base, &connector_basis)
            .unwrap_err()
            .reason,
        "source-helper-layout"
    );

    let mut changed_matrix = connector.clone();
    changed_matrix.assets.source_skeleton.nodes[2].local_rest =
        SourceNodeLocalRest::Matrix(Mat4::from_translation(Vec3::new(0.1, 0.0, 0.0)));
    let changed_matrix = compatibility(&changed_matrix, 0, 1);
    assert_eq!(
        require_assembly_scale_compatibility_with_selectors(&connector_basis, &changed_matrix)
            .unwrap_err()
            .reason,
        "source-helper-rest-basis"
    );

    let reindexed_nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 10,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(0), 20, Vec3::new(0.0, 100.0, 0.0)),
    ];
    let mut reindexed = rig_document(&reindexed_nodes, &[1], 7, Mat4::IDENTITY);
    reindexed.assets.source_skeleton.nodes[1].parent_source_node_index = Some(30);
    let mut reindexed_connector =
        SourceNodeAsset::new(30, SourceNodeLocalRest::Matrix(Mat4::IDENTITY));
    reindexed_connector.parent_source_node_index = Some(10);
    reindexed
        .assets
        .source_skeleton
        .nodes
        .push(reindexed_connector);
    let reindexed = compatibility(&reindexed, 7, 20);
    require_assembly_scale_compatibility_with_selectors(&connector_basis, &reindexed)
        .expect("equal named helper paths remain compatible across format-local indices");

    let indexed_plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
        document: &document,
        capability: &capability,
    })
    .unwrap();
    let indexed = assembly_scale_compatibility_basis(
        &document,
        &indexed_plan,
        AssemblyScaleSelectorRequest::Indexed,
    )
    .unwrap();
    assert_eq!(
        require_assembly_scale_compatibility_with_selectors(&indexed, &base)
            .unwrap_err()
            .reason,
        "source-selector-mode"
    );
}

// --- Whole-document conversion ------------------------------------

#[test]
fn whole_document_factor_one_is_a_literal_no_op() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.0 },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    assert_eq!(
        candidate.document().skeleton.bones[1].rest.translation,
        Vec3::new(0.0, 1.0, 0.0)
    );
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert!(proof.rest_translation.max() < 1e-9);
    assert!(proof.bounds.max() < 1e-6);
}

#[test]
fn preserve_exact_rotation_rejects_signed_zero_and_quaternion_sign_changes() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let plan = whole_document_plan(&doc, &complete_capability());
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    for rotation in [
        Quat::from_xyzw(-0.0, 0.0, 0.0, 1.0),
        Quat::from_xyzw(-0.0, -0.0, -0.0, -1.0),
    ] {
        let mut changed = candidate.document().clone();
        let source_node = changed
            .assets
            .source_skeleton
            .nodes
            .iter_mut()
            .find(|node| node.bone == Some(0))
            .unwrap();
        let SourceNodeLocalRest::Trs {
            rotation: candidate_rotation,
            ..
        } = &mut source_node.local_rest
        else {
            panic!("fixture source root must use TRS")
        };
        *candidate_rotation = rotation;
        assert_eq!(
            prove_scale(&doc, &ScaleCandidate { document: changed }, &plan).unwrap_err(),
            ScaleError::CandidateStructureMismatch {
                reason: "field_disposition_mismatch"
            }
        );
    }
}

#[test]
fn whole_document_conversion_scales_translation_mesh_and_ibm() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let child = &candidate.document().skeleton.bones[1];
    assert!((child.rest.translation - Vec3::new(0.0, 0.01, 0.0)).length() < 1e-6);
    assert_eq!(child.rest.scale, Vec3::ONE);
    let mesh_position = candidate.document().assets.meshes[0].primitives[0].positions[0];
    assert!((mesh_position - Vec3::new(0.01, 0.0, 0.0)).length() < 1e-6);
    let ibm = candidate.document().assets.instances[0].skin_ibms[0];
    assert!(ibm.w_axis.abs_diff_eq(Vec4::new(0.0, 0.0, 0.0, 1.0), 1e-6));
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert!(proof.bounds.max() < 1e-6);
}

#[test]
fn a_candidate_wrapped_from_an_external_document_is_proved_rather_than_trusted() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    })
    .unwrap();

    // Converted here by hand, exactly as a format frontend's reloaded
    // artifact arrives: never through `build_scale_candidate`.
    let mut converted = doc.clone();
    for bone in &mut converted.skeleton.bones {
        bone.rest.translation *= 0.01;
    }
    for node in &mut converted.assets.source_skeleton.nodes {
        node.local_rest = match &node.local_rest {
            SourceNodeLocalRest::Trs {
                translation,
                rotation,
                scale,
            } => SourceNodeLocalRest::Trs {
                translation: *translation * 0.01,
                rotation: *rotation,
                scale: *scale,
            },
            SourceNodeLocalRest::Matrix(matrix) => {
                SourceNodeLocalRest::Matrix(scale_translation_only(*matrix, 0.01))
            }
        };
    }
    for mesh in &mut converted.assets.meshes {
        for primitive in &mut mesh.primitives {
            for position in &mut primitive.positions {
                *position *= 0.01;
            }
        }
    }
    for instance in &mut converted.assets.instances {
        for inverse_bind in &mut instance.skin_ibms {
            *inverse_bind = scale_translation_only(*inverse_bind, 0.01);
        }
    }
    let proof = prove_scale(&doc, &ScaleCandidate::from_document(converted), &plan).unwrap();
    assert!(proof.rest_translation.max() < 1e-6);
    assert!(proof.mesh_position.max() < 1e-6);

    // The constructor asserts nothing, and does not need to: the same
    // wrapper around an *unconverted* document is rejected by proof.
    let error = prove_scale(&doc, &ScaleCandidate::from_document(doc.clone()), &plan)
        .expect_err("an unconverted candidate must not prove");
    assert!(
        matches!(
            error,
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::MeshPosition,
                ..
            }
        ),
        "expected a MeshPosition residual, got {error:?}"
    );
}

#[test]
fn whole_document_conversion_scales_translation_track_values_and_cubic_tangents() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![
                Vec3::new(0.0, -1.0, 0.0), // in-tangent @0
                Vec3::new(0.0, 1.0, 0.0),  // value @0
                Vec3::new(0.0, 1.0, 0.0),  // out-tangent @0
                Vec3::new(0.0, -2.0, 0.0), // in-tangent @1
                Vec3::new(0.0, 2.0, 0.0),  // value @1
                Vec3::new(0.0, 2.0, 0.0),  // out-tangent @1
            ]),
        }],
    });
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let TrackValues::Vec3s(values) = &candidate.document().clips[0].tracks[0].values else {
        panic!("expected vec3 track");
    };
    let expected: Vec<Vec3> = [-1.0, 1.0, 1.0, -2.0, 2.0, 2.0]
        .into_iter()
        .map(|y: f32| Vec3::new(0.0, y * 0.01, 0.0))
        .collect();
    for (value, expected) in values.iter().zip(expected) {
        assert!((*value - expected).length() < 1e-6);
    }
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert!(proof.cubic_interior.max() < 1e-4);
    assert!(proof.trajectory.max() < 1e-4);
    assert!(proof.sample_time_count > 0);
}

// --- Rest/bind selector resolution ----------------------------------

#[test]
fn rest_bind_resolves_shuffled_source_selectors_to_the_correct_bone_closure() {
    // Source-node and source-skin numbering deliberately disagrees with
    // bone-id order: this is the fixture that actually exercises source
    // selector resolution rather than assuming source order == bone order.
    let nodes = vec![
        rig(None, 7, Vec3::ZERO),                  // bone 0 (root), source 7
        rig(Some(0), 2, Vec3::new(0.0, 1.0, 0.0)), // bone 1 (joint), source 2
    ];
    let doc = rig_document(&nodes, &[1], 42, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 42,
            source_root_node_index: 7,
            expected_factor: 1.0,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    assert_eq!(plan.affected_nodes(), &[0, 1]);
}

#[test]
fn rest_bind_factor_one_on_unit_rig_is_a_deterministic_no_op() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.clips.push(Clip {
        name: "scale".into(),
        duration_s: 0.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Scale,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![Vec3::new(2.0, 3.0, 4.0)]),
        }],
    });
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    assert_eq!(
        candidate.document().skeleton.bones[1].rest.translation,
        Vec3::new(0.0, 1.0, 0.0)
    );
    let TrackValues::Vec3s(values) = &candidate.document().clips[0].tracks[0].values else {
        panic!("expected vec3 scale track");
    };
    assert_eq!(values, &[Vec3::new(2.0, 3.0, 4.0)]);
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert!(proof.rest_translation.max() < 1e-9);
}

#[test]
fn rest_bind_requesting_a_different_factor_on_unit_rig_rejects_as_factor_mismatch() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.5,
        },
        document: &doc,
        capability: &capability,
    };
    let error = plan_scale(&request).unwrap_err();
    assert!(matches!(error, ScaleError::FactorMismatch { .. }));
}

// --- Compensated inherited scale + transform-only attachment --------

fn compensated_rig() -> Vec<RigNode> {
    vec![
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
            // A non-identity rotation on the path to the transform-only
            // attachment: DESIGN.md Appendix D §D.3 case 2 requires this
            // so a translation+rotation-only proof cannot mistake a
            // no-op for a correct rebase.
            rotation: Quat::from_rotation_y(0.2),
            scale: Vec3::ONE,
        },
        rig(Some(1), 2, Vec3::new(1.0, 0.0, 0.0)),
    ]
}

fn compensated_document() -> Document {
    let nodes = compensated_rig();
    let child_world = Mat4::from_scale_rotation_translation(
        nodes[0].scale,
        nodes[1].rotation,
        Vec3::new(0.0, 1.0, 0.0),
    );
    let ibm = child_world.inverse();
    rig_document(&nodes, &[1], 0, ibm)
}

/// Replace the direct raw edge from projected bone 0 to projected bone 1
/// with the supplied unprojected matrix rows. `raw_child` and
/// `normalized_child` deliberately remain separate: the former is the
/// authored local below the connectors, while the latter is their
/// collapsed `H * L` representation in the normalized skeleton.
fn compensated_document_with_connectors(
    connectors: &[Mat4],
    raw_child: Transform,
    normalized_child: Transform,
) -> Document {
    assert!(!connectors.is_empty());
    let mut doc = compensated_document();
    doc.skeleton.bones[1].rest = normalized_child;

    let child_source = doc
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .expect("the fixture projects bone 1");
    child_source.local_rest = SourceNodeLocalRest::Trs {
        translation: raw_child.translation,
        rotation: raw_child.rotation,
        scale: raw_child.scale,
    };
    child_source.parent_source_node_index = Some(10 + connectors.len() - 1);

    for (offset, &matrix) in connectors.iter().enumerate() {
        let mut connector = SourceNodeAsset::new(10 + offset, SourceNodeLocalRest::Matrix(matrix));
        connector.parent_source_node_index = Some(if offset == 0 { 0 } else { 9 + offset });
        doc.assets.source_skeleton.nodes.push(connector);
    }

    let root = &doc.skeleton.bones[0].rest;
    let root_world =
        Mat4::from_scale_rotation_translation(root.scale, root.rotation, root.translation);
    let child_local = Mat4::from_scale_rotation_translation(
        normalized_child.scale,
        normalized_child.rotation,
        normalized_child.translation,
    );
    doc.assets.instances[0].skin_ibms[0] = (root_world * child_local).inverse();
    doc
}

fn matrix_with_columns(x: Vec4, y: Vec4, z: Vec4, translation: Vec3) -> Mat4 {
    Mat4::from_cols(x, y, z, translation.extend(1.0))
}

fn assert_source_local_rest_exact(actual: &SourceNodeLocalRest, expected: &SourceNodeLocalRest) {
    match (actual, expected) {
        (SourceNodeLocalRest::Matrix(actual), SourceNodeLocalRest::Matrix(expected)) => {
            assert_eq!(
                actual.to_cols_array().map(f32::to_bits),
                expected.to_cols_array().map(f32::to_bits)
            );
        }
        (
            SourceNodeLocalRest::Trs {
                translation: actual_translation,
                rotation: actual_rotation,
                scale: actual_scale,
            },
            SourceNodeLocalRest::Trs {
                translation: expected_translation,
                rotation: expected_rotation,
                scale: expected_scale,
            },
        ) => {
            assert_eq!(
                actual_translation.to_array().map(f32::to_bits),
                expected_translation.to_array().map(f32::to_bits)
            );
            assert_eq!(
                actual_rotation.to_array().map(f32::to_bits),
                expected_rotation.to_array().map(f32::to_bits)
            );
            assert_eq!(
                actual_scale.to_array().map(f32::to_bits),
                expected_scale.to_array().map(f32::to_bits)
            );
        }
        _ => panic!("source local-rest representation changed"),
    }
}

#[test]
fn rest_bind_rebases_a_projected_successor_through_one_unchanged_connector() {
    // H = T(50, 0, 0) * diag(-2, -2, 2), L = T(100, 0, 0) * S(.5).
    // The normalized child stores H * L = T(-150, 0, 0) * Rz(pi).
    // Moving the 0.01 correction through unchanged H requires the raw
    // successor translation 25.75; the old multiplier-only rewrite would
    // produce 1.0 and is therefore decisively distinguished.
    let connector = matrix_with_columns(
        // Matrix connectors have their own byte-exact preservation arm;
        // retain a negative zero so reconstructing the matrix numerically
        // cannot satisfy that raw write-set claim.
        Vec4::new(-2.0, -0.0, 0.0, 0.0),
        Vec4::new(0.0, -2.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 2.0, 0.0),
        Vec3::new(50.0, 0.0, 0.0),
    );
    let raw_child = Transform {
        translation: Vec3::new(100.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(0.5),
    };
    let normalized_child = Transform {
        translation: Vec3::new(-150.0, 0.0, 0.0),
        rotation: Quat::from_rotation_z(std::f32::consts::PI),
        scale: Vec3::ONE,
    };
    let doc = compensated_document_with_connectors(&[connector], raw_child, normalized_child);
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    assert!(!plan.affected_nodes().contains(&10));

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let connector_after = candidate
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.source_node_index == 10)
        .unwrap();
    assert_source_local_rest_exact(
        &connector_after.local_rest,
        &SourceNodeLocalRest::Matrix(connector),
    );
    let child_after = candidate
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(1))
        .unwrap();
    assert_source_local_rest_exact(
        &child_after.local_rest,
        &SourceNodeLocalRest::Trs {
            translation: Vec3::new(25.75, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.5),
        },
    );
    assert_eq!(
        candidate.document().skeleton.bones[1].rest.translation,
        Vec3::new(-1.5, 0.0, 0.0)
    );
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.rest_translation.comparisons(), 3);
    assert_eq!(proof.rest_rotation.comparisons(), 3);
    assert_eq!(proof.unit_scale.comparisons(), 3);
    assert_eq!(proof.transform_only_affine.comparisons(), 1);
    assert_eq!(proof.skin_matrix.comparisons(), 1);
    assert_eq!(proof.bounds.comparisons(), 6);
    assert!(proof.rest_translation.max() < 1e-4);
    assert!(proof.rest_rotation.max() <= plan.tolerance_policy().rotation_residual_radians);
    assert!(proof.unit_scale.max() <= plan.tolerance_policy().postcondition_unit_scale_residual);
    assert!(proof.skin_matrix.max() < 1e-4);
    assert!(proof.bounds.max() < 1e-4);

    let mut changed_connector = candidate.document().clone();
    let connector = changed_connector
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.source_node_index == 10)
        .unwrap();
    let SourceNodeLocalRest::Matrix(matrix) = &mut connector.local_rest else {
        panic!("fixture connector changed representation");
    };
    matrix.w_axis.x += 1.0;
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate::from_document(changed_connector),
            &plan,
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "connector_source_local_mismatch"
        }
    );

    let mut changed_matrix_zero = candidate.document().clone();
    let SourceNodeLocalRest::Matrix(matrix) = &mut changed_matrix_zero
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.source_node_index == 10)
        .unwrap()
        .local_rest
    else {
        panic!("fixture connector changed representation");
    };
    assert_eq!(matrix.x_axis.y.to_bits(), (-0.0f32).to_bits());
    matrix.x_axis.y = 0.0;
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate::from_document(changed_matrix_zero),
            &plan,
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "connector_source_local_mismatch"
        }
    );

    let mut unrebased_successor = candidate.document().clone();
    unrebased_successor
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .local_rest = doc
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .local_rest
        .clone();
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate::from_document(unrebased_successor),
            &plan,
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "bridged_source_local_mismatch"
        }
    );
}

#[test]
fn rest_bind_composes_and_preserves_a_trs_connector() {
    let raw_child = Transform {
        translation: Vec3::new(100.0, 0.0, 0.0),
        ..Transform::default()
    };
    let normalized_child = Transform {
        translation: Vec3::new(150.0, 0.0, 0.0),
        ..Transform::default()
    };
    let mut doc = compensated_document_with_connectors(
        &[Mat4::from_translation(Vec3::new(50.0, 0.0, 0.0))],
        raw_child,
        normalized_child,
    );
    let connector_before = SourceNodeLocalRest::Trs {
        // The connector write set does not own authored float bits. Keep
        // a noncanonical zero and the negative identity quaternion sign
        // so value equality cannot stand in for the promised byte-exact
        // preservation.
        translation: Vec3::new(50.0, -0.0, 0.0),
        rotation: Quat::from_xyzw(0.0, 0.0, 0.0, -1.0),
        scale: Vec3::ONE,
    };
    doc.assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.source_node_index == 10)
        .unwrap()
        .local_rest = connector_before.clone();

    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let connector_after = candidate
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.source_node_index == 10)
        .unwrap();
    assert_source_local_rest_exact(&connector_after.local_rest, &connector_before);
    let child_after = candidate
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(1))
        .unwrap();
    assert_source_local_rest_exact(
        &child_after.local_rest,
        &SourceNodeLocalRest::Trs {
            translation: Vec3::new(-48.5, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    assert_eq!(
        candidate.document().skeleton.bones[1].rest.translation,
        Vec3::new(1.5, 0.0, 0.0)
    );
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.rest_translation.comparisons(), 3);
    assert_eq!(proof.unit_scale.comparisons(), 3);

    let mut changed_connector = candidate.document().clone();
    let SourceNodeLocalRest::Trs { translation, .. } = &mut changed_connector
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.source_node_index == 10)
        .unwrap()
        .local_rest
    else {
        panic!("fixture connector changed representation");
    };
    assert_eq!(translation.y.to_bits(), (-0.0f32).to_bits());
    translation.y = 0.0;
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate::from_document(changed_connector),
            &plan,
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "connector_source_local_mismatch"
        }
    );

    let mut changed_quaternion_sign = candidate.document().clone();
    let SourceNodeLocalRest::Trs { rotation, .. } = &mut changed_quaternion_sign
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.source_node_index == 10)
        .unwrap()
        .local_rest
    else {
        panic!("fixture connector changed representation");
    };
    assert_eq!(rotation.w.to_bits(), (-1.0f32).to_bits());
    *rotation = Quat::IDENTITY;
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate::from_document(changed_quaternion_sign),
            &plan,
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "connector_source_local_mismatch"
        }
    );

    let mut changed_successor = candidate.document().clone();
    let SourceNodeLocalRest::Trs { translation, .. } = &mut changed_successor
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .local_rest
    else {
        panic!("fixture successor changed representation");
    };
    assert_eq!(translation.z, 0.0);
    translation.z = f32::from_bits(translation.z.to_bits() ^ 0x8000_0000);
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate::from_document(changed_successor),
            &plan,
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "bridged_source_local_mismatch"
        }
    );

    let mut adjacent_successor = candidate.document().clone();
    let SourceNodeLocalRest::Trs { translation, .. } = &mut adjacent_successor
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .local_rest
    else {
        panic!("fixture successor changed representation");
    };
    translation.x = f32::from_bits(translation.x.to_bits() + 1);
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate::from_document(adjacent_successor),
            &plan,
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "bridged_source_local_mismatch"
        }
    );

    let mut changed_successor_rotation = candidate.document().clone();
    let SourceNodeLocalRest::Trs { rotation, .. } = &mut changed_successor_rotation
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .local_rest
    else {
        panic!("fixture successor changed representation");
    };
    *rotation = Quat::from_xyzw(0.0, 0.0, 0.0, -1.0);
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate::from_document(changed_successor_rotation),
            &plan,
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "bridged_source_local_mismatch"
        }
    );
}

#[test]
fn rest_bind_composes_a_connector_between_two_non_root_projected_joints() {
    let mut doc = compensated_document();
    let connector_before = SourceNodeLocalRest::Trs {
        translation: Vec3::new(50.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    let mut connector = SourceNodeAsset::new(10, connector_before.clone());
    connector.parent_source_node_index = Some(1);
    doc.assets.source_skeleton.nodes.push(connector);

    let successor_before = SourceNodeLocalRest::Trs {
        translation: Vec3::new(-49.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    let successor = doc
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(2))
        .unwrap();
    successor.parent_source_node_index = Some(10);
    successor.local_rest = successor_before.clone();

    doc.assets.source_skeleton.skins[0]
        .joint_source_node_indices
        .push(2);
    doc.assets.instances[0].skin_joints.push(2);
    let bone_two_world = doc.skeleton.bones[..=2]
        .iter()
        .fold(Mat4::IDENTITY, |world, bone| {
            world
                * Mat4::from_scale_rotation_translation(
                    bone.rest.scale,
                    bone.rest.rotation,
                    bone.rest.translation,
                )
        });
    doc.assets.instances[0]
        .skin_ibms
        .push(bone_two_world.inverse());

    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let source_nodes = &candidate.document().assets.source_skeleton.nodes;
    assert_source_local_rest_exact(
        &source_nodes
            .iter()
            .find(|node| node.source_node_index == 10)
            .unwrap()
            .local_rest,
        &connector_before,
    );
    assert_source_local_rest_exact(
        &source_nodes
            .iter()
            .find(|node| node.bone == Some(2))
            .unwrap()
            .local_rest,
        &SourceNodeLocalRest::Trs {
            translation: Vec3::new(-49.0, 0.0, 0.0) * 0.01 + Vec3::new(-49.5, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    assert_eq!(
        candidate.document().skeleton.bones[2].rest.translation,
        Vec3::new(0.01, 0.0, 0.0)
    );
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn rest_bind_multiplies_three_unchanged_connectors_in_parent_to_child_order() {
    // H1 has linear 2*Rz(90) and translation (10, 0); H2 has linear
    // .5*Rz(-90) and translation (0, 20). In authored order H1*H2 is
    // exactly T(-30, 0). H3 adds T(5, 0), so the full product is
    // T(-25, 0) and H1*H2*H3*T(25, 40) collapses to T(0, 40).
    // Reversing or omitting any helper produces a different translation.
    let h1 = matrix_with_columns(
        Vec4::new(0.0, 2.0, 0.0, 0.0),
        Vec4::new(-2.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 2.0, 0.0),
        Vec3::new(10.0, 0.0, 0.0),
    );
    let h2 = matrix_with_columns(
        Vec4::new(0.0, -0.5, 0.0, 0.0),
        Vec4::new(0.5, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 0.5, 0.0),
        Vec3::new(0.0, 20.0, 0.0),
    );
    let h3 = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
    let raw_child = Transform {
        translation: Vec3::new(25.0, 40.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    let normalized_child = Transform {
        translation: Vec3::new(0.0, 40.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    let doc = compensated_document_with_connectors(&[h1, h2, h3], raw_child, normalized_child);
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    for (source, expected) in [(10, h1), (11, h2), (12, h3)] {
        let connector_after = candidate
            .document()
            .assets
            .source_skeleton
            .nodes
            .iter()
            .find(|node| node.source_node_index == source)
            .unwrap();
        assert_source_local_rest_exact(
            &connector_after.local_rest,
            &SourceNodeLocalRest::Matrix(expected),
        );
    }
    let child_after = candidate
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(1))
        .unwrap();
    assert_source_local_rest_exact(
        &child_after.local_rest,
        &SourceNodeLocalRest::Trs {
            translation: Vec3::new(25.0, 0.39999998, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    assert_eq!(
        candidate.document().skeleton.bones[1].rest.translation,
        Vec3::new(0.0, 0.39999998, 0.0)
    );
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn compensated_connector_product_is_widened_before_the_f32_candidate_boundary() {
    // The full projected source chain stays finite because the .01 root
    // and 1e-40 successor compensate the two 1e20 connectors. The
    // connector-only product is 1e40, however, and therefore overflows a
    // Mat4 before the compensation can be applied. Planning already
    // accepts the finite endpoint; build and proof must retain that same
    // domain by carrying H in f64 until the final source-local result.
    let huge = Mat4::from_scale(Vec3::splat(1e20));
    let raw_child = Transform {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(1e-40),
    };
    let doc = compensated_document_with_connectors(&[huge, huge], raw_child, Transform::default());
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let successor = candidate
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(1))
        .unwrap();
    assert_source_local_rest_exact(
        &successor.local_rest,
        &SourceNodeLocalRest::Trs {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(1e-40),
        },
    );
    prove_scale(&doc, &candidate, &plan).unwrap();

    // The candidate retains Complete source projection, so its normalized
    // factor-one chain must remain a consumable planner input even though
    // the preserved connector-only product is still above f32 range.
    let normalized_plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document: candidate.document(),
        capability: &capability,
    })
    .unwrap();
    let normalized_candidate =
        build_scale_candidate(candidate.document(), &normalized_plan).unwrap();
    prove_scale(
        candidate.document(),
        &normalized_candidate,
        &normalized_plan,
    )
    .unwrap();
}

#[test]
fn compensated_connector_translation_is_widened_before_the_f32_candidate_boundary() {
    // The selected root's .01 factor keeps every complete raw source-world
    // intermediate finite, while the connector-only product contains the
    // translations 1e40, -2e40, and 3e40. The projected successor cancels
    // them with finite authored values on every axis. Accumulating any
    // connector translation lane in f32 would overflow before that
    // compensation.
    let connector_scale = Mat4::from_scale(Vec3::splat(1e20));
    let connector_translation = Mat4::from_translation(Vec3::new(1e20, -2e20, 3e20));
    let raw_child = Transform {
        translation: Vec3::new(-1e20, 2e20, -3e20),
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(1e-20),
    };
    let doc = compensated_document_with_connectors(
        &[connector_scale, connector_translation],
        raw_child,
        Transform::default(),
    );
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let successor = candidate
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(1))
        .unwrap();
    let SourceNodeLocalRest::Trs { translation, .. } = &successor.local_rest else {
        panic!("fixture successor changed representation");
    };
    assert_eq!(translation.x.to_bits(), (-1e20f32).to_bits());
    assert_eq!(translation.y.to_bits(), 2e20f32.to_bits());
    assert_eq!(translation.z.to_bits(), (-3e20f32).to_bits());
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn bridged_translation_terms_are_combined_before_the_f32_candidate_boundary() {
    // Each term of the bridged successor translation exceeds f32 even
    // though their analytic sum is exactly representable and differs
    // from the authored value on every axis. Narrowing the direct term or
    // bridge offset separately produces `inf + -inf`; the complete
    // expression must stay in f64 until its single final narrowing.
    let factor = 128.0;
    let connector_scale = f32::from_bits(0x3c00_0000); // 2^-7
    let magnitude = f32::from_bits(0x7b00_0000); // 2^119
    let connector_translation = Vec3::new(-magnitude, magnitude, -magnitude);
    let authored_successor_translation =
        Vec3::new(magnitude * 129.0, -magnitude * 129.0, magnitude * 130.0);
    let normalized_successor_translation = Vec3::new(
        f32::from_bits(0x7780_0000), // 2^112
        f32::from_bits(0xf780_0000), // -2^112
        f32::from_bits(0x7800_0000), // 2^113
    );
    let expected_successor_translation = Vec3::new(
        f32::from_bits(0x7f00_0000), // 2^127
        f32::from_bits(0xff00_0000), // -2^127
        f32::from_bits(0x7f40_0000), // 3 * 2^126
    );
    let connector = Mat4::from_scale_rotation_translation(
        Vec3::splat(connector_scale),
        Quat::IDENTITY,
        connector_translation,
    );
    let authored_successor = Transform {
        translation: authored_successor_translation,
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(factor),
    };
    let normalized_successor = Transform {
        translation: normalized_successor_translation,
        ..Transform::default()
    };
    let expected_successor = Transform {
        translation: expected_successor_translation,
        ..authored_successor
    };
    for matrix_successor in [false, true] {
        let mut doc = compensated_document_with_connectors(
            &[connector],
            authored_successor,
            normalized_successor,
        );
        doc.skeleton.bones[0].rest.scale = Vec3::splat(factor);
        let SourceNodeLocalRest::Trs { scale, .. } = &mut doc
            .assets
            .source_skeleton
            .nodes
            .iter_mut()
            .find(|node| node.bone == Some(0))
            .unwrap()
            .local_rest
        else {
            panic!("fixture root changed representation");
        };
        *scale = Vec3::splat(factor);
        doc.assets.instances[0].skin_ibms[0] = Mat4::from_scale_rotation_translation(
            Vec3::splat(connector_scale),
            Quat::IDENTITY,
            -normalized_successor_translation,
        );

        let expected_successor = if matrix_successor {
            SourceNodeLocalRest::Matrix(Mat4::from_scale_rotation_translation(
                expected_successor.scale,
                expected_successor.rotation,
                expected_successor.translation,
            ))
        } else {
            SourceNodeLocalRest::Trs {
                translation: expected_successor.translation,
                rotation: expected_successor.rotation,
                scale: expected_successor.scale,
            }
        };
        if matrix_successor {
            doc.assets
                .source_skeleton
                .nodes
                .iter_mut()
                .find(|node| node.bone == Some(1))
                .unwrap()
                .local_rest = SourceNodeLocalRest::Matrix(authored_successor.to_mat4());
        }

        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: f64::from(factor),
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let successor = candidate
            .document()
            .assets
            .source_skeleton
            .nodes
            .iter()
            .find(|node| node.bone == Some(1))
            .unwrap();
        assert_source_local_rest_exact(&successor.local_rest, &expected_successor);
        prove_scale(&doc, &candidate, &plan).unwrap();
    }
}

#[test]
fn bridged_matrix_linear_ratio_is_combined_before_the_f32_candidate_boundary() {
    // The raw Matrix successor's 2^125 linear terms are compensated by
    // the connector's 2^-125 scale. Both projected endpoint factors are
    // 128, so the successor's required local ratio is exactly one. A
    // sequential f32 `linear * s_parent * (1 / s_node)` overflows at the
    // first multiplication even though the exact candidate is the
    // authored finite matrix.
    let factor = 128.0;
    let connector_scale = f32::from_bits(0x0100_0000); // 2^-125
    let successor_scale = f32::from_bits(0x7e00_0000); // 2^125
    let authored_successor = Transform {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(successor_scale),
    };
    let authored_successor_matrix = authored_successor.to_mat4();
    let mut doc = compensated_document_with_connectors(
        &[Mat4::from_scale(Vec3::splat(connector_scale))],
        authored_successor,
        Transform::default(),
    );
    doc.skeleton.bones[0].rest.scale = Vec3::splat(factor);
    let SourceNodeLocalRest::Trs { scale, .. } = &mut doc
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(0))
        .unwrap()
        .local_rest
    else {
        panic!("fixture root changed representation");
    };
    *scale = Vec3::splat(factor);
    doc.assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .local_rest = SourceNodeLocalRest::Matrix(authored_successor_matrix);
    doc.assets.instances[0].skin_ibms[0] = Mat4::from_scale(Vec3::splat(1.0 / factor));

    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: f64::from(factor),
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let successor = candidate
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(1))
        .unwrap();
    assert_source_local_rest_exact(
        &successor.local_rest,
        &SourceNodeLocalRest::Matrix(authored_successor_matrix),
    );
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn planning_composes_connector_translation_into_the_raw_source_world() {
    // Both connector locals are finite, but the selected root followed by
    // S(1e20) * T(f32::MAX) has a non-finite world translation. Dropping
    // connector translation while retaining its linear part would make
    // the compensated successor appear to be a valid .01 endpoint.
    let connector_scale = Mat4::from_scale(Vec3::splat(1e20));
    let connector_translation = Mat4::from_translation(Vec3::splat(f32::MAX));
    let doc = compensated_document_with_connectors(
        &[connector_scale, connector_translation],
        Transform {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(1e-20),
        },
        Transform::default(),
    );
    let capability = complete_capability();
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap_err(),
        ScaleError::NonFiniteSourceTransform {
            source_node_index: 1
        }
    );
}

#[test]
fn rest_bind_classifies_projected_endpoints_not_nonuniform_connector_rows() {
    let connector = Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0));
    let raw_child = Transform {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::new(0.5, 1.0 / 3.0, 0.25),
    };
    let normalized_child = Transform::default();
    let mut doc = compensated_document_with_connectors(&[connector], raw_child, normalized_child);
    doc.assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.source_node_index == 10)
        .unwrap()
        .local_rest = SourceNodeLocalRest::Trs {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::new(2.0, 3.0, 4.0),
    };
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn rest_bind_refuses_a_projective_unprojected_connector() {
    let projective =
        matrix_with_columns(Vec4::new(1.0, 0.0, 0.0, 0.25), Vec4::Y, Vec4::Z, Vec3::ZERO);
    let rest = Transform::default();
    let doc = compensated_document_with_connectors(&[projective], rest, rest);
    let capability = complete_capability();
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap_err(),
        ScaleError::IncompleteClosure {
            reason: "non_affine_connector_source_transform"
        }
    );
}

#[test]
fn bridged_matrix_successor_preserves_linear_and_homogeneous_bits() {
    let connector = Mat4::from_translation(Vec3::new(50.0, 0.0, 0.0));
    let raw_child_matrix = Mat4::from_cols(
        Vec4::X,
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(100.0, 0.0, 0.0, 1.0),
    );
    let mut doc = compensated_document_with_connectors(
        &[connector],
        Transform {
            translation: Vec3::new(100.0, 0.0, 0.0),
            ..Transform::default()
        },
        Transform {
            translation: Vec3::new(150.0, 0.0, 0.0),
            ..Transform::default()
        },
    );
    doc.assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .local_rest = SourceNodeLocalRest::Matrix(raw_child_matrix);
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let SourceNodeLocalRest::Matrix(rebased) = candidate
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .local_rest
    else {
        panic!("matrix successor changed representation");
    };
    assert_eq!(rebased.x_axis, raw_child_matrix.x_axis);
    assert_eq!(rebased.y_axis, raw_child_matrix.y_axis);
    assert_eq!(rebased.z_axis, raw_child_matrix.z_axis);
    assert_eq!(
        Vec4::new(
            rebased.x_axis.w,
            rebased.y_axis.w,
            rebased.z_axis.w,
            rebased.w_axis.w,
        ),
        Vec4::new(0.0, 0.0, 0.0, 1.0)
    );
    assert_eq!(rebased.w_axis.truncate(), Vec3::new(-48.5, 0.0, 0.0));
    prove_scale(&doc, &candidate, &plan).unwrap();

    let mut changed_successor_linear = candidate.document().clone();
    let SourceNodeLocalRest::Matrix(matrix) = &mut changed_successor_linear
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .local_rest
    else {
        panic!("matrix successor changed representation");
    };
    assert_eq!(matrix.x_axis.y.to_bits(), 0.0f32.to_bits());
    matrix.x_axis.y = -0.0;
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate::from_document(changed_successor_linear),
            &plan,
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "bridged_source_local_mismatch"
        }
    );
}

#[test]
fn connector_bridge_factor_one_is_a_public_bit_exact_no_op() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let connector_before = SourceNodeLocalRest::Trs {
        translation: Vec3::new(50.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    let mut connector = SourceNodeAsset::new(10, connector_before.clone());
    connector.parent_source_node_index = Some(0);
    doc.assets.source_skeleton.nodes.push(connector);
    let child_before = SourceNodeLocalRest::Trs {
        translation: Vec3::new(-50.0, 1.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    let child = doc
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .unwrap();
    child.parent_source_node_index = Some(10);
    child.local_rest = child_before.clone();

    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let source_nodes = &candidate.document().assets.source_skeleton.nodes;
    assert_source_local_rest_exact(
        &source_nodes
            .iter()
            .find(|node| node.source_node_index == 10)
            .unwrap()
            .local_rest,
        &connector_before,
    );
    assert_source_local_rest_exact(
        &source_nodes
            .iter()
            .find(|node| node.bone == Some(1))
            .unwrap()
            .local_rest,
        &child_before,
    );
    assert_eq!(
        candidate.document().skeleton.bones[1].rest.translation,
        Vec3::new(0.0, 1.0, 0.0)
    );
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn a_finite_but_unrepresentable_connector_rebase_is_atomic() {
    // H and L compensate to a finite projected endpoint, so planning
    // accepts the document. Moving the .01 basis through H requires an
    // approximately -9.9e39 raw translation, which cannot be represented
    // by the f32 source-local model boundary and must fail before a
    // candidate can escape.
    let connector = Mat4::from_scale_rotation_translation(
        Vec3::splat(1e-20),
        Quat::IDENTITY,
        Vec3::splat(1e20),
    );
    let doc = compensated_document_with_connectors(
        &[connector],
        Transform {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(1e20),
        },
        Transform {
            translation: Vec3::splat(1e20),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    assert_eq!(
        build_scale_candidate(&doc, &plan).unwrap_err(),
        ScaleError::NonFiniteTransform { node: 1 }
    );
    assert_source_local_rest_exact(
        &doc.assets
            .source_skeleton
            .nodes
            .iter()
            .find(|node| node.source_node_index == 10)
            .unwrap()
            .local_rest,
        &SourceNodeLocalRest::Matrix(connector),
    );
    assert_eq!(doc.skeleton.bones[1].rest.translation, Vec3::splat(1e20));
}

#[test]
fn a_shared_connector_chain_plans_builds_and_proves_through_public_apis() {
    const CONNECTORS: usize = 64;
    const PROJECTED_CHILDREN: usize = 64;
    // The first child fills the connector-product cache and every later
    // sibling must reuse the same nonidentity T(50) product. Returning an
    // identity or a partial product on cache hits changes the exact
    // successor local below.
    let normalized_child_translation = Vec3::new(150.0, 0.0, 0.0);
    let raw_child_translation = Vec3::new(100.0, 0.0, 0.0);
    let expected_child_translation = Vec3::new(-48.5, 0.0, 0.0);
    let mut nodes = vec![RigNode {
        parent: None,
        source_node_index: 0,
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(0.01),
    }];
    nodes.extend(
        (1..=PROJECTED_CHILDREN).map(|source| rig(Some(0), source, normalized_child_translation)),
    );
    let skin_bones: Vec<BoneId> = (1..=PROJECTED_CHILDREN).collect();
    let child_world =
        Mat4::from_scale(Vec3::splat(0.01)) * Mat4::from_translation(normalized_child_translation);
    let mut doc = rig_document(&nodes, &skin_bones, 0, child_world.inverse());
    for offset in 0..CONNECTORS {
        let source = 100 + offset;
        let matrix = if offset == 0 {
            Mat4::from_translation(Vec3::new(50.0, 0.0, 0.0))
        } else {
            Mat4::IDENTITY
        };
        let mut connector = SourceNodeAsset::new(source, SourceNodeLocalRest::Matrix(matrix));
        connector.parent_source_node_index = Some(if offset == 0 { 0 } else { source - 1 });
        doc.assets.source_skeleton.nodes.push(connector);
    }
    let tail = 100 + CONNECTORS - 1;
    for child in doc
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .filter(|node| node.bone.is_some_and(|bone| bone != 0))
    {
        child.parent_source_node_index = Some(tail);
        child.local_rest = SourceNodeLocalRest::Trs {
            translation: raw_child_translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
    }

    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    assert_eq!(plan.affected_nodes().len(), 1 + PROJECTED_CHILDREN);
    let domain = derive_rest_bind_plan_domain(&doc, 0, 0).unwrap();
    assert!(
        domain.ancestry_steps <= CONNECTORS + 1,
        "shared connector ancestry was walked {} times",
        domain.ancestry_steps
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    for source in 100..100 + CONNECTORS {
        let before = doc
            .assets
            .source_skeleton
            .nodes
            .iter()
            .find(|node| node.source_node_index == source)
            .unwrap();
        let after = candidate
            .document()
            .assets
            .source_skeleton
            .nodes
            .iter()
            .find(|node| node.source_node_index == source)
            .unwrap();
        assert_source_local_rest_exact(&after.local_rest, &before.local_rest);
    }
    for child in candidate
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .filter(|node| node.bone.is_some_and(|bone| bone != 0))
    {
        assert_source_local_rest_exact(
            &child.local_rest,
            &SourceNodeLocalRest::Trs {
                translation: expected_child_translation,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
    }
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn a_stale_plan_cannot_replay_across_connector_topology_changes() {
    let identity = Mat4::IDENTITY;
    let rest = Transform::default();
    let doc = compensated_document_with_connectors(&[identity, identity], rest, rest);
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);

    let mut reordered = doc.clone();
    reordered
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.source_node_index == 10)
        .unwrap()
        .parent_source_node_index = Some(11);
    reordered
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.source_node_index == 11)
        .unwrap()
        .parent_source_node_index = Some(0);
    reordered
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .parent_source_node_index = Some(10);
    assert_eq!(
        build_scale_candidate(&reordered, &plan).unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "affected_source_topology_mismatch"
        }
    );
    assert_eq!(
        prove_scale(
            &reordered,
            &ScaleCandidate::from_document(reordered.clone()),
            &plan,
        )
        .unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "affected_source_topology_mismatch"
        }
    );

    let mut changed = doc.clone();
    changed.assets.source_skeleton.nodes.push(SourceNodeAsset {
        source_node_index: 12,
        name: None,
        parent_source_node_index: Some(11),
        scene_root_indices: Vec::new(),
        local_rest: SourceNodeLocalRest::Matrix(Mat4::IDENTITY),
        bone: None,
    });
    changed
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(1))
        .unwrap()
        .parent_source_node_index = Some(12);
    assert_eq!(
        build_scale_candidate(&changed, &plan).unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "affected_source_topology_mismatch"
        }
    );
    assert_eq!(
        prove_scale(
            &changed,
            &ScaleCandidate::from_document(changed.clone()),
            &plan,
        )
        .unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "affected_source_topology_mismatch"
        }
    );
}

#[test]
fn a_replayed_plan_allows_connector_numeric_changes_when_topology_is_identical() {
    let rest = Transform::default();
    let doc = compensated_document_with_connectors(&[Mat4::IDENTITY], rest, rest);
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);

    let mut numerically_changed = doc.clone();
    numerically_changed
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.source_node_index == 10)
        .unwrap()
        .local_rest = SourceNodeLocalRest::Matrix(Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));
    let candidate = build_scale_candidate(&numerically_changed, &plan).unwrap();
    prove_scale(&numerically_changed, &candidate, &plan).unwrap();
}

#[test]
fn compensated_inherited_scale_reparameterizes_and_preserves_world_geometry() {
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    assert_eq!(plan.transform_only_attachments(), &[2]);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let bones = &candidate.document().skeleton.bones;
    assert!((bones[0].rest.scale - Vec3::ONE).length() < 1e-6);
    assert!((bones[1].rest.translation - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-6);
    // Transform-only attachment: rebased from (1,0,0) to (0.01,0,0).
    assert!((bones[2].rest.translation - Vec3::new(0.01, 0.0, 0.0)).length() < 1e-6);

    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert!(proof.rest_translation.max() < 1e-4);
    assert!(proof.unit_scale.max() < 1e-4);
    assert!(proof.transform_only_affine.max() < 1e-4);
    assert!(proof.skin_matrix.max() < 1e-3);
}

#[test]
fn a_stale_no_op_candidate_for_the_transform_only_attachment_fails_proof() {
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    // Simulate a builder bug that left the transform-only attachment's
    // local translation un-rebased.
    broken.skeleton.bones[2].rest.translation = Vec3::new(1.0, 0.0, 0.0);
    let broken = ScaleCandidate { document: broken };
    assert!(prove_scale(&doc, &broken, &plan).is_err());
}

// --- A closure that genuinely branches -------------------------------

/// A rest/bind rig whose affected closure *branches*, at two depths.
///
/// Every other rig in this module is a single chain: no domain node has
/// more than one child, so a descendant walk that followed only each
/// node's *first* child would still produce the correct closure for all
/// of them, and the traversal's breadth goes entirely unpinned.
///
/// ```text
/// bone 0  source 0  parent -  T (0, 0, 0)     S 0.01  scaled root
/// bone 1  source 1  parent 0  T (0, 100, 0)   S 1     the skin's only joint
/// bone 2  source 2  parent 0  T (100, 0, 0)   S 1     root's SECOND child
/// bone 3  source 3  parent 1  T (0, 0, 100)   S 1     joint's first child
/// bone 4  source 4  parent 1  T (0, 50, 0)    S 1     joint's SECOND child
/// bone 5  source 5  parent 2  T (0, 0, 50)    S 1     child of bone 2 only
/// ```
///
/// Bones 2, 4 and 5 are reachable *only* through a second-or-later
/// child: none of them is a skin joint, and none lies on a joint's
/// ancestor path, so neither the root insertion nor the joint
/// ancestor walk can pull them in — only the descendant walk can, and
/// only if it visits more than one child per node. Bone 5 additionally
/// hangs below a second child, so it is reachable only after the walk
/// has already branched once.
///
/// Hand-computed rest-world matrices — linear part `0.01 * I`
/// throughout, so the whole domain classifies at the common factor
/// `0.01`:
///
/// ```text
/// W0 (0, 0, 0)   W1 (0, 1, 0)     W2 (1, 0, 0)
/// W3 (0, 1, 1)   W4 (0, 1.5, 0)   W5 (1, 0, 0.5)
/// ```
fn branching_rig() -> Vec<RigNode> {
    vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
        rig(Some(0), 2, Vec3::new(100.0, 0.0, 0.0)),
        rig(Some(1), 3, Vec3::new(0.0, 0.0, 100.0)),
        rig(Some(1), 4, Vec3::new(0.0, 50.0, 0.0)),
        rig(Some(2), 5, Vec3::new(0.0, 0.0, 50.0)),
    ]
}

fn branching_document() -> Document {
    // `B1 = inverse(W1) = inverse([0.01 I | (0, 1, 0)]) = [100 I | (0, -100, 0)]`,
    // written as a literal rather than derived from the fixture, so
    // `W1 * B1 = I` is a hand-checked fact of the source.
    let ibm = Mat4::from_cols(
        Vec4::new(100.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 100.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 100.0, 0.0),
        Vec4::new(0.0, -100.0, 0.0, 1.0),
    );
    let mut doc = rig_document(&branching_rig(), &[1], 0, ibm);
    // Animated on the branch that only a second child reaches: bone 4's
    // world translation is `(0, 1, 0) + 0.01 * value`, so the source
    // trajectory runs `(0, 1.5, 0)` -> `(0, 1.6, 0)`.
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 4,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::new(0.0, 50.0, 0.0), Vec3::new(0.0, 60.0, 0.0)]),
        }],
    });
    doc
}

#[test]
fn a_branching_affected_closure_pulls_in_every_child_at_every_depth() {
    let doc = branching_document();
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    // The complete expected closure, as a literal. A walk that follows
    // only a first child yields `[0, 1, 3]`: bone 2 (the root's second
    // child), bone 4 (the joint's second child) and bone 5 (below bone
    // 2) are all missing.
    assert_eq!(plan.affected_nodes(), &[0, 1, 2, 3, 4, 5]);
    // Everything but the scaled root and the skin's one joint.
    assert_eq!(plan.transform_only_attachments(), &[2, 3, 4, 5]);
    assert_eq!(plan.common_factor(), 0.01);

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let bones = &candidate.document().skeleton.bones;
    // `L' = C_parent^-1 * L * C_i`: every affected local translation is
    // multiplied by its parent's factor (`0.01` inside the domain, one
    // at the root boundary) and every affected local scale becomes one.
    let expected_local = [
        Vec3::ZERO,
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 0.5, 0.0),
        Vec3::new(0.0, 0.0, 0.5),
    ];
    for (node, expected) in expected_local.iter().enumerate() {
        assert!(
            (bones[node].rest.translation - *expected).length() < 1e-6,
            "bone {node} translation {:?}",
            bones[node].rest.translation
        );
        assert!(
            (bones[node].rest.scale - Vec3::ONE).length() < 1e-6,
            "bone {node} scale {:?}",
            bones[node].rest.scale
        );
    }
    // The animated branch node is rebased by its parent's `0.01` too.
    let TrackValues::Vec3s(values) = &candidate.document().clips[0].tracks[0].values else {
        panic!("expected a vec3 track");
    };
    assert!((values[0] - Vec3::new(0.0, 0.5, 0.0)).length() < 1e-6);
    assert!((values[1] - Vec3::new(0.0, 0.6, 0.0)).length() < 1e-6);
    // `B1' = C^-1 * B1 = scale(0.01) * [100 I | (0, -100, 0)]
    //      = [I | (0, -1, 0)]`.
    let rebased_ibm = candidate.document().assets.instances[0].skin_ibms[0];
    assert!(
        rebased_ibm.abs_diff_eq(
            Mat4::from_cols(
                Vec4::new(1.0, 0.0, 0.0, 0.0),
                Vec4::new(0.0, 1.0, 0.0, 0.0),
                Vec4::new(0.0, 0.0, 1.0, 0.0),
                Vec4::new(0.0, -1.0, 0.0, 1.0),
            ),
            1e-6
        ),
        "rebased inverse bind {rebased_ibm:?}"
    );

    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    // Two key times, from the one animated branch node.
    assert_eq!(proof.sample_time_count, 2);
    assert!(proof.rest_translation.max() < 1e-6);
    assert!(proof.unit_scale.max() < 1e-6);
    assert!(proof.transform_only_affine.max() < 1e-6);
    assert!(proof.trajectory.max() < 1e-6);
    assert!(proof.key_translation.max() < 1e-6);
    assert!(proof.skin_matrix.max() < 1e-4);
    assert!(proof.bounds.max() < 1e-6);
}

// --- The `C_parent = I` boundary at the top of the domain ------------

/// A rest/bind rig whose scaled root is *not* the skeleton root.
///
/// DESIGN.md Appendix D §D.2 rebases each affected local matrix as
/// `L' = C_parent^-1 * L * C_i`, where `C_i = scale(1 / s)` inside the
/// affected domain and `C_parent = I` at its parent boundary. Every other
/// fixture here scales the skeleton root itself, with a zero local
/// translation and no track of its own, so `C_parent = I` and
/// `C_parent = C_i` are indistinguishable and the boundary rule — the
/// core of the operation — goes unpinned on both the build and the proof
/// side.
///
/// This rig makes them differ, in one closure:
///
/// ```text
/// bone 0   parent -   T (5, 0, 0)     S 1      boundary parent, outside
/// bone 1   parent 0   T (0, 2, 0)     S 0.01   scaled root, animated
/// bone 2   parent 1   T (0, 100, 0)   S 1      the skin's only joint
/// bone 3   parent 2   T (100, 0, 0)   S 1      attachment, depth 1
/// bone 4   parent 3   T (0, 0, 200)   S 1      attachment, depth 2
/// ```
///
/// Every rest-world linear part from bone 1 down is `0.01 * I`, so the
/// domain classifies at the common factor `0.01`; bone 0 contributes a
/// pure translation and is neither a joint, an ancestor path between
/// joints, nor a descendant, so it stays outside. The rest-world
/// translations are bone 1 `(5, 2, 0)`, bone 2 `(5, 3, 0)`, bone 3
/// `(6, 3, 0)` and bone 4 `(6, 3, 2)`.
fn parent_boundary_rig() -> Vec<RigNode> {
    vec![
        rig(None, 0, Vec3::new(5.0, 0.0, 0.0)),
        RigNode {
            parent: Some(0),
            source_node_index: 1,
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(1), 2, Vec3::new(0.0, 100.0, 0.0)),
        rig(Some(2), 3, Vec3::new(100.0, 0.0, 0.0)),
        rig(Some(3), 4, Vec3::new(0.0, 0.0, 200.0)),
    ]
}

fn parent_boundary_document() -> Document {
    // `B = inverse(W_rest(bone 2))` for `W = T(5, 3, 0) * scale(0.01)`:
    // `W^-1 = scale(100) * T(-5, -3, 0)`, that is a linear part of
    // `100 * I` and a translation column of `100 * (-5, -3, 0)`.
    let ibm = Mat4::from_scale_rotation_translation(
        Vec3::splat(100.0),
        Quat::IDENTITY,
        Vec3::new(-500.0, -300.0, 0.0),
    );
    let mut doc = rig_document(&parent_boundary_rig(), &[2], 0, ibm);
    // A translation track on the scaled root *itself*. Its parent is
    // outside the closure, so this track's parent-basis multiplier is the
    // boundary factor of one and both values must survive the rebase
    // byte-for-byte — unlike the descendant tracks every other animated
    // fixture here carries, which are rebased by `s`.
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 4.0, 0.0)]),
        }],
    });
    doc
}

#[test]
fn a_scaled_root_whose_parent_is_outside_the_closure_keeps_its_own_translation_basis() {
    let doc = parent_boundary_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    // The closure is the scaled root, its one joint, and *both*
    // attachment levels below it: bone 4 is two hops below the deepest
    // node the joint/ancestor seeding reaches, so a descendant walk that
    // stops after one level drops it. Bone 0 is the boundary parent and
    // must stay out.
    assert_eq!(plan.affected_nodes(), &[1, 2, 3, 4]);
    assert_eq!(plan.transform_only_attachments(), &[3, 4]);
    assert_eq!(plan.common_factor(), 0.01);
    // The observed factor is measured *at the scaled root*, and here that
    // is bone 1, not bone 0. Bone 0's rest-world linear part is the
    // identity, so an implementation that measured a fixed bone zero — or
    // the first affected bone by any rule other than "the plan's resolved
    // scaled root" — would report `1.0`, a hundredfold error, rather than
    // `0.01f32` widened to `f64`. Every other observed-factor fixture in
    // this module puts its scaled root at bone 0 and cannot separate the
    // two.
    assert_eq!(plan.observed_factor(), NEAR_UNIT_OBSERVED_FACTOR);

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let bones = &candidate.document().skeleton.bones;
    // The boundary parent is not in the domain and is not rewritten.
    assert_eq!(bones[0].rest.translation, Vec3::new(5.0, 0.0, 0.0));
    assert_eq!(bones[0].rest.scale, Vec3::ONE);
    // Scaled root: `C_parent = I`, so its local translation keeps the
    // basis it was authored in — multiplying it by `s` here would move
    // its world origin from `(5, 2, 0)` to `(5, 0.02, 0)` — while its own
    // `C_i` still corrects its local scale from `0.01` to one.
    assert!((bones[1].rest.translation - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-9);
    // Below the root every parent is itself affected, so
    // `C_parent = scale(1 / s)` and each local translation is rebased by
    // `s = 0.01`: `(0, 100, 0) -> (0, 1, 0)`, `(100, 0, 0) -> (1, 0, 0)`,
    // `(0, 0, 200) -> (0, 0, 2)`.
    assert!((bones[2].rest.translation - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-6);
    assert!((bones[3].rest.translation - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-6);
    assert!((bones[4].rest.translation - Vec3::new(0.0, 0.0, 2.0)).length() < 1e-6);
    for (id, bone) in bones.iter().enumerate().skip(1) {
        assert!(
            (bone.rest.scale - Vec3::ONE).length() < 1e-6,
            "bone {id} local scale {:?}",
            bone.rest.scale
        );
    }

    // The raw source projection is rebased by the same rule, and the
    // scaled root's authored local translation is unchanged there too.
    let source_nodes = &candidate.document().assets.source_skeleton.nodes;
    let expected_projection = [
        (0, Vec3::new(5.0, 0.0, 0.0), Vec3::ONE),
        (1, Vec3::new(0.0, 2.0, 0.0), Vec3::ONE),
        (2, Vec3::new(0.0, 1.0, 0.0), Vec3::ONE),
        (3, Vec3::new(1.0, 0.0, 0.0), Vec3::ONE),
        (4, Vec3::new(0.0, 0.0, 2.0), Vec3::ONE),
    ];
    for (index, expected_translation, expected_scale) in expected_projection {
        let SourceNodeLocalRest::Trs {
            translation, scale, ..
        } = &source_nodes[index].local_rest
        else {
            panic!("expected a trs source rest");
        };
        assert!(
            (*translation - expected_translation).length() < 1e-6,
            "source node {index} translation {translation:?}"
        );
        assert!(
            (*scale - expected_scale).length() < 1e-6,
            "source node {index} scale {scale:?}"
        );
    }

    // The scaled root's own translation track is *not* rebased.
    let TrackValues::Vec3s(values) = &candidate.document().clips[0].tracks[0].values else {
        panic!("expected a vec3 track");
    };
    let expected_values = [Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 4.0, 0.0)];
    for (value, expected) in values.iter().zip(expected_values) {
        assert!((*value - expected).length() < 1e-9, "track value {value:?}");
    }

    // `B' = C^-1 * B = scale(s) * B`: linear `I`, translation column
    // `0.01 * (-500, -300, 0)`.
    let binds = &candidate.document().assets.instances[0].skin_ibms;
    assert_eq!(binds.len(), 1);
    assert!(
        binds[0].abs_diff_eq(Mat4::from_translation(Vec3::new(-5.0, -3.0, 0.0)), 1e-5),
        "rebased bind {:?}",
        binds[0]
    );

    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    // Re-derived independently of the plan, and at the same bone 1.
    assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
    // Two key times, no cubic segment.
    assert_eq!(proof.sample_time_count, 2);
    assert!(proof.rest_translation.max() < 1e-4);
    assert!(proof.unit_scale.max() < 1e-4);
    assert!(proof.transform_only_affine.max() < 1e-4);
    // Exactly zero: the one rewritten track's multiplier is one.
    assert!(proof.track_value.max() < 1e-9);
    assert!(proof.trajectory.max() < 1e-4);
    assert!(proof.skin_matrix.max() < 1e-4);
    assert!(proof.bounds.max() < 1e-4);
}

/// A rig whose scaled root sits under an ancestor that is itself scaled
/// and stays *outside* the closure, so the root's local scale and its
/// composed rest-world scale are different numbers:
///
/// ```text
///   bone 0  local scale 2      world scale 2      (boundary parent)
///   bone 1  local scale 0.005  world scale 0.01   (scaled root)
///   bone 2  local scale 1      world scale 0.01   (the skin's joint)
/// ```
///
/// Every step is exact in binary32: `0.005f32` is `0.01f32 / 2` exactly
/// (halving is exact for a normal number), so `2 * 0.005f32 == 0.01f32`
/// and the root's composed linear part is `0.01f32 * I` with no rounding
/// at all. The observed factor is therefore the same
/// [`NEAR_UNIT_OBSERVED_FACTOR`] every `0.01f32` fixture here reports,
/// while the root's *local* scale widens to `0.004999999888241291` —
/// half of it.
fn scaled_ancestor_rig() -> Vec<RigNode> {
    vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::new(5.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(2.0),
        },
        RigNode {
            parent: Some(0),
            source_node_index: 1,
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.005),
        },
        rig(Some(1), 2, Vec3::new(0.0, 100.0, 0.0)),
    ]
}

fn scaled_ancestor_document() -> Document {
    // `B = inverse(W_rest(bone 2))` for `W = T(5, 5, 0) * scale(0.01)`:
    // a linear part of `100 * I` and a translation column of
    // `100 * (-5, -5, 0)`.
    let ibm = Mat4::from_scale_rotation_translation(
        Vec3::splat(100.0),
        Quat::IDENTITY,
        Vec3::new(-500.0, -500.0, 0.0),
    );
    rig_document(&scaled_ancestor_rig(), &[2], 0, ibm)
}

#[test]
fn the_observed_factor_is_the_scaled_roots_composed_scale_not_its_local_one() {
    // DESIGN.md Appendix D §D.1 classifies the affine domain from each
    // node's *rest-world* linear part, and §D.6's observed factor is that
    // measurement at the scaled root. Every other fixture in this module
    // puts the scaled root directly under an unscaled parent, where the
    // local and composed scales coincide and an implementation that read
    // `Bone::rest.scale` — or the raw projection's local `Trs` scale —
    // is indistinguishable from one that composed the world matrix. Here
    // they differ by exactly the factor two the boundary parent carries.
    let doc = scaled_ancestor_document();
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    // The scaled ancestor is the boundary parent and stays out.
    assert_eq!(plan.affected_nodes(), &[1, 2]);
    assert_eq!(plan.common_factor(), 0.01);
    assert_eq!(plan.observed_factor(), NEAR_UNIT_OBSERVED_FACTOR);
    // The local scale a local-only measurement would have reported, named
    // here so the two are separated by an exact comparison rather than by
    // a tolerance: it is half the composed factor.
    assert_eq!(
        f64::from(doc.skeleton.bones[1].rest.scale.x),
        NEAR_UNIT_OBSERVED_FACTOR / 2.0
    );

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    // `C_parent = I` at the boundary, `C_1 = scale(1 / 0.01f32)`, and
    // `1.0f32 / 0.01f32` is exactly `100.0`, so the root's local scale
    // becomes `0.005f32 * 100` — which rounds to exactly `0.5`, leaving
    // the composed root scale `2 * 0.5 = 1` with a zero residual.
    assert_eq!(
        candidate.document().skeleton.bones[1].rest.scale,
        Vec3::splat(0.5)
    );
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.unit_scale.max(), 0.0);
    assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
}

// --- Affine violation classes (rest/bind classification) -----------

/// Two-node rig used for affine-violation fixtures: `mutate` edits the
/// root's raw source local-rest matrix, which is what classification
/// now runs against (not the lossy TRS-decomposed `Bone::rest`).
fn reject_case(mutate: impl FnOnce(&mut SourceNodeLocalRest)) -> ScaleError {
    let nodes = vec![rig(None, 0, Vec3::ZERO), rig(Some(0), 1, Vec3::ZERO)];
    let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    let root = &mut doc.assets.source_skeleton.nodes[0].local_rest;
    mutate(root);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    plan_scale(&request).unwrap_err()
}

fn trs_scale(scale: Vec3) -> SourceNodeLocalRest {
    SourceNodeLocalRest::Trs {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale,
    }
}

#[test]
fn the_equal_axis_band_is_relative_to_the_longer_axis_and_admits_its_own_edge() {
    // `classify_affine` rejects an axis whose length is farther from the
    // three-axis average than `equal_axis * average.max(length)`. Two
    // things about that comparison are untested by every other fixture,
    // because they all sit far from the edge with the long axis on the
    // same side: the base is the `max` and not the `average`, and the
    // comparison is `>` and not `>=`.
    //
    // One fixture pins both. All three axis lengths are exact in binary32
    // and the average is exact in binary64:
    //
    //   lengths = (99998.5, 99998.5, 100000.0)
    //   average = 299997.0 / 3           = 99999.0   (exact)
    //   longest axis deviation           = 100000.0 - 99999.0 = 1.0
    //   1e-5 * max(average, 100000.0)    = 1e-5 * 100000.0 = 1.0 exactly
    //   1e-5 * average                   = 1e-5 * 99999.0  = 0.99999
    //
    // so `1.0 > 1.0` is false and the basis classifies as uniform, while
    // an `average` base (`1.0 > 0.99999`) or a `>=` comparison
    // (`1.0 >= 1.0`) both reject it as `NonUniformScale`. The two short
    // axes are `0.5` from the average against the same `0.99999`, so
    // neither of them decides anything here.
    //
    // `1e-5 * 100000.0` is exactly `1.0` in binary64: `fl(1e-5)` exceeds
    // `1e-5` by `8.18e-22`, so the product exceeds one by `8.18e-17`,
    // inside the `1.11e-16` half-ulp at one.
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::new(99_998.5, 99_998.5, 100_000.0),
        },
        rig(Some(0), 1, Vec3::ZERO),
    ];
    let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = plan_scale(&declared_factor_request(&doc, &capability, 99_999.0))
        .expect("an axis exactly on the equal-axis edge is uniform");
    // The declared factor `99_999.0` is exactly the axis-length average
    // `classify_affine` returns, so the plan is accepted with a zero
    // factor residual and nothing downstream of the equal-axis check can
    // be what admitted it. That the average differs from the longest axis
    // at all is what makes the `max` base observable here.
    assert_eq!(plan.common_factor(), 99_999.0);

    // One binary32 ulp of the long axis past the edge. At `100_000.0` that
    // ulp is `2^-7 = 0.0078125`, so the length becomes `100000.0078125`,
    // the average `99999.0026041666...`, and the deviation
    // `1.0052083333...` against a `1.000000078125` band.
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::new(99_998.5, 99_998.5, 100_000.0 + 0.007_812_5),
        },
        rig(Some(0), 1, Vec3::ZERO),
    ];
    let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    assert!(matches!(
        plan_scale(&declared_factor_request(&doc, &capability, 99_999.0)).unwrap_err(),
        ScaleError::InvalidAffineDomain {
            reason: AffineDomainViolation::NonUniformScale,
            ..
        }
    ));
}

#[test]
fn an_orthogonality_dot_exactly_on_its_tolerance_is_accepted_and_the_next_one_up_is_not() {
    // `classify_affine` rejects shear on `dot.abs() > relative_orthogonality
    // * average * average`. Distinguishing that `>` from `>=` needs a
    // binary32 basis whose binary64 column dot lands *exactly* on the
    // tolerance. This is that basis. It was found by search, but the
    // arithmetic below is what makes it checkable without rerunning one:
    // every literal is an exact dyadic written as `mantissa * 2^-k`, so
    // the binary32 bits are readable straight off the page.
    //
    //   col0 = (1,  y0, 0)     y0 = 13_826_165 * 2^-69
    //   col1 = (x1, 1,  0)     x1 = 10_995_118 * 2^-40
    //   col2 = (0,  0,  c)     c  =  8_388_610 * 2^-23
    //
    // Two columns lie in the xy-plane and the third is the z-axis, so
    // `dot02` and `dot12` are exactly zero and only `dot01` decides.
    //
    //   dot01 = 1*x1 + y0*1 + 0*0 = x1 + y0
    //
    // and that sum is *exact* in binary64: `x1` is a multiple of `2^-40`,
    // `y0` a multiple of `2^-69`, and the sum is just under `2^-16`, so it
    // needs 53 bits and gets them. `y0` is small enough that `y0 * y0`
    // vanishes against one, so `col0`'s length is exactly `1.0` and moving
    // `y0` does not move the tolerance at all — it moves only `dot01`, and
    // one binary32 step of `y0` is `2^-69`, which is exactly one binary64
    // ulp of `dot01` in its `[2^-17, 2^-16)` binade. The three cases below
    // are therefore three consecutive representable dot products against
    // one fixed tolerance.
    //
    // Accepting the middle one and rejecting the next proves the middle
    // one *is* the tolerance rather than merely below it: acceptance gives
    // `dot <= tolerance`, rejection of `dot + 1ulp` gives
    // `tolerance < dot + 1ulp`, and the only binary64 in `[dot, dot+1ulp)`
    // is `dot` itself. So `>=` would reject the middle case, and does.
    let c = 8_388_610.0_f32 * 2.0_f32.powi(-23);
    let x1 = 10_995_118.0_f32 * 2.0_f32.powi(-40);
    let y0 = 13_826_165.0_f32 * 2.0_f32.powi(-69);
    let y0_up = 13_826_166.0_f32 * 2.0_f32.powi(-69);
    let y0_down = 13_826_164.0_f32 * 2.0_f32.powi(-69);
    // The fixture's own claim, checked before it is relied on: the three
    // dot products are consecutive binary64 values.
    let dot = f64::from(x1) + f64::from(y0);
    assert_eq!(
        (f64::from(x1) + f64::from(y0_up)).to_bits(),
        dot.to_bits() + 1
    );
    assert_eq!(
        (f64::from(x1) + f64::from(y0_down)).to_bits(),
        dot.to_bits() - 1
    );

    let basis = |y: f32| {
        Mat3::from_cols(
            Vec3::new(1.0, y, 0.0),
            Vec3::new(x1, 1.0, 0.0),
            Vec3::new(0.0, 0.0, c),
        )
    };
    let tol = ScaleTolerancePolicy::APPENDIX_D_V6;
    assert!(classify_affine(basis(y0_down), &tol).is_ok());
    assert!(classify_affine(basis(y0), &tol).is_ok());
    assert_eq!(
        classify_affine(basis(y0_up), &tol),
        Err(AffineDomainViolation::Sheared)
    );
}

#[test]
fn appendix_d_v6_rejects_the_shared_policy_divergence_fixture() {
    assert_eq!(
        classify_affine(
            crate::model::affine_test_fixtures::tolerance_divergence_basis(),
            &ScaleTolerancePolicy::APPENDIX_D_V6,
        ),
        Err(AffineDomainViolation::NonUniformScale)
    );
}

#[test]
fn affine_adapter_maps_each_named_policy_field_to_its_own_classifier_band() {
    let strict_shape_policy = ScaleTolerancePolicy {
        equal_axis: 1.0e-4,
        relative_orthogonality: 1.0e-5,
        singular_determinant_relative: 1.0e-7,
        ..ScaleTolerancePolicy::APPENDIX_D_V6
    };

    assert!(
        classify_affine(
            crate::model::affine_test_fixtures::tolerance_divergence_basis(),
            &strict_shape_policy,
        )
        .is_ok(),
        "the loose equal-axis band must reach the equal-axis field"
    );
    assert_eq!(
        classify_affine(
            crate::model::affine_test_fixtures::orthogonality_tolerance_divergence_basis(),
            &strict_shape_policy,
        ),
        Err(AffineDomainViolation::Sheared),
        "the strict orthogonality band must reach the orthogonality field"
    );
    let loose_orthogonality_policy = ScaleTolerancePolicy {
        relative_orthogonality: 1.0e-4,
        ..strict_shape_policy
    };
    assert!(
        classify_affine(
            crate::model::affine_test_fixtures::orthogonality_tolerance_divergence_basis(),
            &loose_orthogonality_policy,
        )
        .is_ok(),
        "the loose orthogonality band must reach the orthogonality field"
    );

    let determinant_boundary_basis =
        |determinant: f32| Mat3::from_cols(Vec3::X, Vec3::new(1.0, determinant, 0.0), Vec3::Z);
    assert_eq!(
        classify_affine(determinant_boundary_basis(5.0e-8), &strict_shape_policy),
        Err(AffineDomainViolation::Singular),
        "the strict singularity band must reach the singularity field"
    );
    assert_eq!(
        classify_affine(determinant_boundary_basis(5.0e-7), &strict_shape_policy),
        Err(AffineDomainViolation::Sheared),
        "a determinant beyond the singularity band must reach the later shape check"
    );
    assert_eq!(
        classify_affine(
            determinant_boundary_basis(5.0e-7),
            &ScaleTolerancePolicy::APPENDIX_D_V6,
        ),
        Err(AffineDomainViolation::Singular),
        "the production singularity band must differ from the strict test policy"
    );
}

#[test]
fn nonuniform_scale_domain_rejects() {
    let error = reject_case(|rest| *rest = trs_scale(Vec3::new(0.01, 0.02, 0.01)));
    assert!(matches!(
        error,
        ScaleError::InvalidAffineDomain {
            reason: AffineDomainViolation::NonUniformScale,
            ..
        }
    ));
}

#[test]
fn rest_bind_planning_refuses_every_appendix_d_v6_mean_permutation() {
    // The shared helper's exact fixture lies on the association-sensitive
    // equal-axis boundary. Exercise the public producer boundary, not
    // just the classifier: no accepted plan means no candidate can be
    // constructed or handed to `prove_scale` under a different factor.
    for (permutation, linear) in
        crate::model::affine_test_fixtures::appendix_d_v6_mean_permutations()
            .into_iter()
            .enumerate()
    {
        let error = reject_case(|rest| {
            *rest = SourceNodeLocalRest::Matrix(Mat4::from_cols(
                linear.x_axis.extend(0.0),
                linear.y_axis.extend(0.0),
                linear.z_axis.extend(0.0),
                Vec4::W,
            ));
        });
        assert_eq!(
            error,
            ScaleError::InvalidAffineDomain {
                node: 0,
                reason: AffineDomainViolation::NonUniformScale,
            },
            "orientation-preserving permutation {permutation}"
        );
    }
}

#[test]
fn literal_shear_via_a_raw_matrix_fixture_rejects() {
    // A TRS-only check can never see this: `SourceNodeLocalRest::Matrix`
    // is the only representation that carries a literal shear term. All
    // three columns keep equal length (so the uniform-axis check alone
    // cannot explain the rejection) but the first two are not
    // orthogonal, isolating the shear violation.
    let angle = 80f32.to_radians();
    let error = reject_case(|rest| {
        *rest = SourceNodeLocalRest::Matrix(Mat4::from_cols_array(&[
            1.0,
            0.0,
            0.0,
            0.0,
            angle.cos(),
            angle.sin(),
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        ]));
    });
    assert!(matches!(
        error,
        ScaleError::InvalidAffineDomain {
            reason: AffineDomainViolation::Sheared,
            ..
        }
    ));
}

#[test]
fn reflected_domain_rejects() {
    let error = reject_case(|rest| *rest = trs_scale(Vec3::new(-0.01, 0.01, 0.01)));
    assert!(matches!(
        error,
        ScaleError::InvalidAffineDomain {
            reason: AffineDomainViolation::Reflected,
            ..
        }
    ));
}

#[test]
fn singular_domain_rejects() {
    let error = reject_case(|rest| *rest = trs_scale(Vec3::new(0.0, 0.01, 0.01)));
    assert!(matches!(
        error,
        ScaleError::InvalidAffineDomain {
            reason: AffineDomainViolation::Singular,
            ..
        }
    ));
}

#[test]
fn near_singular_domain_rejects() {
    // Equal-length axes (so the uniform-axis check alone would pass)
    // that are nearly parallel: the determinant collapses toward zero
    // while every column stays unit length.
    let eps = 1e-8f32;
    let error = reject_case(|rest| {
        *rest = SourceNodeLocalRest::Matrix(Mat4::from_cols_array(&[
            1.0,
            0.0,
            0.0,
            0.0,
            eps.cos(),
            eps.sin(),
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        ]));
    });
    assert!(matches!(
        error,
        ScaleError::InvalidAffineDomain {
            reason: AffineDomainViolation::Singular,
            ..
        }
    ));
}

#[test]
fn nonfinite_domain_rejects() {
    // A non-finite raw local-rest transform is caught composing raw
    // source-node world matrices, before affine classification ever
    // runs.
    let error = reject_case(|rest| *rest = trs_scale(Vec3::new(f32::NAN, 0.01, 0.01)));
    assert!(matches!(
        error,
        ScaleError::NonFiniteSourceTransform {
            source_node_index: 0
        }
    ));
}

#[test]
fn mixed_factor_within_domain_rejects() {
    let nodes = vec![rig(None, 0, Vec3::ZERO), rig(Some(0), 1, Vec3::ZERO)];
    let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    doc.assets.source_skeleton.nodes[0].local_rest = trs_scale(Vec3::splat(0.01));
    doc.assets.source_skeleton.nodes[1].local_rest = trs_scale(Vec3::splat(0.02));
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&request).unwrap_err(),
        ScaleError::MixedFactor { .. }
    ));
}

/// A two-node rig whose root carries `scale` in *both* the normalized
/// `Bone::rest` and the raw `source_skeleton` projection, so a plan
/// accepted from the source projection can be carried through
/// `build_scale_candidate` and `prove_scale` without the two disagreeing.
fn noisy_factor_document(scale: f32) -> Document {
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(scale),
        },
        rig(Some(0), 1, Vec3::ZERO),
    ];
    rig_document(&nodes, &[1], 0, Mat4::IDENTITY)
}

fn noisy_factor_request<'a>(
    document: &'a Document,
    capability: &'a ScaleCapabilityFacts,
) -> ScaleRequest<'a> {
    ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document,
        capability,
    }
}

#[test]
fn noisy_but_within_tolerance_factor_is_accepted_and_just_outside_is_not() {
    // DESIGN.md Appendix D §D.3 case 4: a noisy near-factor is accepted
    // only when its measured residual is within the declared tolerance
    // — which §D.1 declares *relative* `1e-5`. The accept/reject deltas
    // are therefore derived from the factor's own magnitude, not from a
    // floored comparison base, and the two fixtures below are the two
    // *adjacent* binary32 values that straddle the band edge, so the
    // boundary itself is pinned rather than merely bracketed:
    //
    //   accept: 0.010_000_099_f32 == 0.010000099427998065948486328125
    //           |s - 0.01|            = 9.9427998065948e-8
    //           1e-5 * max(s, 0.01)   = 1.0000099428e-7  -> inside
    //   reject: 0.010_000_1_f32   == 0.01000010035932064056396484375
    //           |s - 0.01|            = 1.0035932064056e-7
    //           1e-5 * max(s, 0.01)   = 1.0000100359e-7  -> outside
    //
    // The two literals differ by exactly one binary32 ulp at this
    // magnitude (`2^-30 = 9.31322574615478515625e-10`), so no
    // representable factor sits between them.
    for (scale, should_accept) in [(0.010_000_099_f32, true), (0.010_000_1_f32, false)] {
        let doc = noisy_factor_document(scale);
        let capability = complete_capability();
        let request = noisy_factor_request(&doc, &capability);
        let planned = plan_scale(&request);
        assert_eq!(
            planned.is_ok(),
            should_accept,
            "scale {scale} accepted={should_accept}"
        );
        if !should_accept {
            assert!(matches!(
                planned.unwrap_err(),
                ScaleError::FactorMismatch { .. }
            ));
        }
    }
}

/// A request whose declared factor is a free `f64`, so a fixture can put
/// the observed and declared magnitudes either way round at the band edge
/// — the `f32` root scale alone cannot, because one binary32 ulp is far
/// wider than the window the two comparison bases disagree over.
fn declared_factor_request<'a>(
    document: &'a Document,
    capability: &'a ScaleCapabilityFacts,
    expected_factor: f64,
) -> ScaleRequest<'a> {
    ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor,
        },
        document,
        capability,
    }
}

#[test]
fn the_common_factor_band_is_relative_to_the_larger_operand_whichever_it_is() {
    // DESIGN.md Appendix D §D.1 states the band as
    // `abs(a - b) <= c * max(abs(a), abs(b))` — the base is the `max`, not
    // whichever operand `relative` happens to receive first. The existing
    // boundary fixtures all pass the *larger* operand first (an observed
    // factor slightly above the declared one), so a base of `abs(a)`
    // agrees with `max` on every one of them and the `max` is untested.
    //
    // Both rows below sit exactly on the band edge, in the two operand
    // orders. `relative` is called as `relative(c, observed, declared)`,
    // and the observed common factor of a root whose rest scale is a
    // uniform `s` is `s` itself — `classify_affine` averages three column
    // lengths that are all exactly `s`. Every value is exact in binary64:
    //
    //   1e-5 * 100000.0 == 1.0            (exactly; the f64 product of
    //                                      fl(1e-5) and 1e5 rounds to 1)
    //   abs(99999.0 - 100000.0) == 1.0    (exactly)
    //
    //   smaller first: a = 99999,  b = 100000
    //                  max base  -> 1.0 <= 1e-5 * 100000 = 1.0   accept
    //                  abs(a) base -> 1.0 <= 1e-5 * 99999 = 0.99999
    //                                                            reject
    //   larger first:  a = 100000, b = 99999
    //                  max base  -> 1.0 <= 1e-5 * 100000 = 1.0   accept
    //
    // The second row is the `max` in its already-covered order, but both
    // rows make the *inclusive* `<=` load-bearing: the residual is exactly
    // the bound, so a strict `<` rejects both of them.
    let capability = complete_capability();
    for (observed, declared) in [(99_999.0f32, 100_000.0f64), (100_000.0f32, 99_999.0f64)] {
        let doc = noisy_factor_document(observed);
        let request = declared_factor_request(&doc, &capability, declared);
        let plan = plan_scale(&request).unwrap_or_else(|error| {
            panic!("observed {observed} declared {declared} must plan, got {error:?}")
        });
        // The plan carries the *declared* factor, so this also records
        // that the two really did differ by the full band width rather
        // than the fixture having collapsed them onto one value.
        assert_eq!(plan.common_factor(), declared);
        assert_eq!((f64::from(observed) - declared).abs(), 1.0);
    }

    // One `f64` ulp of the declared factor past the edge, in the
    // smaller-first order, so the row above is a boundary and not merely
    // a wide band. One ulp at `100_000.0` is `2^-36 =
    // 1.4551915228366852e-11`, so the residual becomes `1 + 2^-36` while
    // the base grows by only `1e-5 * 2^-36 = 1.455e-16`, which rounds the
    // tolerance up to `1 + 2^-52`:
    //
    //   1 + 2^-36 = 1.0000000000145519  >  1 + 2^-52 = 1.0000000000000002
    let doc = noisy_factor_document(99_999.0);
    let outside =
        declared_factor_request(&doc, &capability, f64::from_bits(100_000f64.to_bits() + 1));
    assert!(matches!(
        plan_scale(&outside).unwrap_err(),
        ScaleError::FactorMismatch { .. }
    ));
}

#[test]
fn a_noisy_factor_plan_scale_accepts_still_satisfies_its_own_proof_postcondition() {
    // The accept side of the band above is only meaningful if the plan
    // it produces is actually buildable and provable. With the earlier
    // `1.0`-floored comparison base this was false: `plan_scale`
    // accepted a factor `8e-6` relative off, whose candidate then failed
    // its own `UnitScale` postcondition at `1.386e-3` against `1e-5`.
    //
    // Arithmetic for the value below, all exact in binary32:
    //
    //   s              = 0.010_000_02_f32 = 0.0100000202655792236328125
    //   1.0f32/0.01f32 = 100.0 exactly, so the rebased root local scale
    //                    is fl(s * 100) = 1.00000202655792236328125
    //                                   = 1 + 17 * 2^-23
    //   per-axis (L-infinity) unit-scale residual
    //                  = 17 * 2^-23 = 2.02655792236328125e-6
    //
    // An L2 norm over the three axes would report `sqrt(3)` times that
    // (`3.51e-6`), which is why the equality below is exact rather than a
    // one-sided bound: it pins the *norm*, not just the magnitude.
    let doc = noisy_factor_document(0.010_000_02);
    let capability = complete_capability();
    let request = noisy_factor_request(&doc, &capability);
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(
        proof.unit_scale.max(),
        17.0 * 2f64.powi(-23),
        "unit scale residual {}",
        proof.unit_scale.max()
    );
}

// --- Declared vs observed factor (DESIGN.md Appendix D §D.6) ----------

/// `0.010_000_02_f32`, exactly, as a `f64`.
///
/// The rest-world linear part of `noisy_factor_document(0.010_000_02)`'s
/// root is `scale(s) * I`, whose three column lengths are each exactly
/// `s`: `s` needs 24 significant bits, so `s * s` needs at most 48 and is
/// exact in binary64, and `sqrt` of an exact square is exact. Their mean
/// is `s` again — `3 * s` needs at most 26 bits and is exact, and its
/// quotient by three is the representable `s` — so the observed factor is
/// this literal and not a rounded neighbour of it.
///
/// Spelled as the shortest decimal that round-trips to that binary64
/// value, because `clippy::excessive_precision` rejects the full exact
/// expansion. The exact value is `0.0100000202655792236328125`.
const NOISY_OBSERVED_FACTOR: f64 = 0.010_000_020_265_579_224;

/// `0.01_f32`, exactly, as a `f64` — by the same argument as
/// [`NOISY_OBSERVED_FACTOR`], and spelled the same way. The exact value
/// is `0.00999999977648258209228515625`.
const NEAR_UNIT_OBSERVED_FACTOR: f64 = 0.009_999_999_776_482_582;

#[test]
fn a_rest_bind_plan_and_its_proof_both_report_the_observed_factor_and_the_declared_one() {
    // DESIGN.md Appendix D §D.6 requires producer evidence to record "the
    // operation kind, declared **and observed** factors". Planning
    // measures the observed factor to validate the declared one against
    // it, so the two are distinct numbers and both are reachable.
    //
    // The fixture's root rest scale is inside the `1e-5` common-factor
    // band but nowhere near equal to the declared `0.01`, so an
    // implementation that reported the declared factor under either name
    // is separated from this one by an exact comparison:
    //
    //   s                   = 0.0100000202655792236328125
    //   abs(s - 0.01)       = 2.02655792236328125e-8
    //   1e-5 * max(s, 0.01) = 1.0000020265579e-7   -> inside the band
    let doc = noisy_factor_document(0.010_000_02);
    let capability = complete_capability();
    let plan = plan_scale(&noisy_factor_request(&doc, &capability)).unwrap();
    assert_eq!(plan.common_factor(), 0.01);
    assert_eq!(plan.observed_factor(), NOISY_OBSERVED_FACTOR);

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.observed_factor, NOISY_OBSERVED_FACTOR);
}

#[test]
fn the_proof_re_derives_the_observed_factor_from_its_own_source_not_from_the_plan() {
    // The point of §D.6's "observed" column is that proof evidence stands
    // on its own: `prove_scale` must measure the factor from the document
    // it was handed rather than echo what planning recorded. Nothing
    // requires that document to be the one the plan came from — the
    // module says so outright — so planning one source and proving
    // another separates a re-derivation from a copy, and nothing else
    // does: for any single document the two numbers agree.
    //
    // Both roots sit inside the band around the declared `0.01`, so both
    // plan and both prove:
    //
    //   planned root 0.010_000_02_f32 = 0.0100000202655792236328125
    //   proved  root 0.01_f32         = 0.00999999977648258209228515625
    //
    // The proved candidate's root composed scale is exactly one: the
    // build's rebase multiplier is `1.0f32 / 0.01f32 == 100.0` exactly,
    // and `fl(0.01f32 * 100)` is `0.999999977648258209228515625` rounded
    // to binary32 — a `2.235e-8` error against a `2^-25 = 2.98e-8`
    // half-ulp below one, so it rounds to `1.0` and the unit-scale
    // postcondition is met with a zero residual.
    let planned = noisy_factor_document(0.010_000_02);
    let proved = noisy_factor_document(0.01);
    let capability = complete_capability();
    let plan = plan_scale(&noisy_factor_request(&planned, &capability)).unwrap();
    assert_eq!(plan.observed_factor(), NOISY_OBSERVED_FACTOR);

    let candidate = build_scale_candidate(&proved, &plan).unwrap();
    let proof = prove_scale(&proved, &candidate, &plan).unwrap();
    assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
    assert_eq!(proof.unit_scale.max(), 0.0);
}

// --- Reconciling the two observed factors (issue #306) ----------------

#[test]
fn the_evidence_record_carries_both_observed_factors_and_the_divergence_between_them() {
    // §D.6's evidence record carries two numbers that both answer to "the
    // observed factor", measured from genuinely different state. The
    // record therefore also carries how far apart they are, so a consumer
    // neither mistakes one for the other nor has to derive the
    // relationship itself.
    let capability = complete_capability();

    // A document whose two chains agree — the ordinary case — puts both
    // witnesses on the same number, and the divergence is a *measured*
    // zero rather than an unset field.
    let consistent = noisy_factor_document(0.010_000_02);
    let plan = plan_scale(&noisy_factor_request(&consistent, &capability)).unwrap();
    let candidate = build_scale_candidate(&consistent, &plan).unwrap();
    let proof = prove_scale(&consistent, &candidate, &plan).unwrap();
    assert_eq!(proof.planned_observed_factor, NOISY_OBSERVED_FACTOR);
    assert_eq!(proof.observed_factor, NOISY_OBSERVED_FACTOR);
    assert_eq!(proof.observed_factor_divergence, 0.0);

    // Planning one source and proving another separates the two
    // witnesses, exactly as `the_proof_re_derives_the_observed_factor
    // _from_its_own_source_not_from_the_plan` does — and this proof
    // *succeeds*, so the divergence below is a measurement taken from a
    // complete evidence record rather than a number read off a failure.
    //
    //   planned 0.010_000_02_f32 = 10_737_440 * 2^-30
    //   proved  0.01_f32         = 10_737_418 * 2^-30
    //   difference               =         22 * 2^-30 (exact: equal
    //                                      exponents, and the result is
    //                                      representable)
    //   divergence = 22 / 10_737_440 = 11 / 5_368_720
    //              = 2.0489055119283555e-6
    let proved = noisy_factor_document(0.01);
    let candidate = build_scale_candidate(&proved, &plan).unwrap();
    let proof = prove_scale(&proved, &candidate, &plan).unwrap();
    assert_eq!(proof.planned_observed_factor, NOISY_OBSERVED_FACTOR);
    assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
    assert_eq!(proof.observed_factor_divergence, 11.0 / 5_368_720.0);
    // Nowhere near the ceiling: this pair separates the two witnesses,
    // it does not saturate their bands. The fixture below does that.
    assert!(
        proof.observed_factor_divergence
            < plan.tolerance_policy().observed_factor_divergence_ceiling()
    );

    // The same two documents with their roles swapped, so the *proved*
    // witness is now the larger of the pair. The comparison base is the
    // `max` of the two, exactly as `ScaleTolerancePolicy::relative`'s is
    // and for the same reason, so the divergence is the same number in
    // both directions — which a base of "whichever operand came first"
    // would not be. Without this direction that `max` is untested: every
    // other fixture here observes the planned witness as the larger one.
    let swapped_plan = plan_scale(&noisy_factor_request(&proved, &capability)).unwrap();
    let swapped_candidate = build_scale_candidate(&consistent, &swapped_plan).unwrap();
    let swapped = prove_scale(&consistent, &swapped_candidate, &swapped_plan).unwrap();
    assert_eq!(swapped.planned_observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
    assert_eq!(swapped.observed_factor, NOISY_OBSERVED_FACTOR);
    assert_eq!(swapped.observed_factor_divergence, 11.0 / 5_368_720.0);
}

#[test]
fn the_divergence_ceiling_is_the_sum_of_the_two_bands_that_produce_it() {
    // Stated from the two bands rather than read back off the policy, so
    // a ceiling derived from any other pair of fields — the equal-axis
    // band, the scalar relative term, a multiple of either — fails here.
    // The sum is exact: `2^-14` is a power of two well above `fl(1e-5)`'s
    // last bit, and the rounded sum is the same binary64 value the
    // decimal literal denotes.
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    assert_eq!(policy.common_factor, 1e-5);
    assert_eq!(policy.postcondition_unit_scale_residual, 2f64.powi(-14));
    assert_eq!(
        policy.observed_factor_divergence_ceiling(),
        1e-5 + 2f64.powi(-14)
    );
    assert_eq!(
        policy.observed_factor_divergence_ceiling(),
        7.103_515_625e-5
    );
}

#[test]
fn a_pair_that_nearly_spends_both_bands_leaves_barely_any_ceiling_headroom() {
    // Named for what it pins and no wider: *this* pair's divergence and
    // *this* pair's headroom. The general form — "saturating both bands
    // keeps a divergence inside their sum" — is false, and
    // `a_document_whose_skeleton_and_projection_disagree_is_proved_not
    // _refused` below is the counterexample: the rebase's binary32
    // rounding can put a divergence over the sum with both witnesses
    // still honouring their own band. What the two tests together say is
    // that the ceiling is tight in the direction this one measures and
    // not a proved bound in the direction that one does.
    //
    // The ceiling claim of §D.6, exercised at the far end of both bands
    // it is the sum of, on a pair of documents that *proves*:
    //
    //   planned root 0.010_000_099_f32 = 10_737_525 * 2^-30
    //     |s - 0.01| = 9.9427998e-8 against 1e-5 * s = 1.00000994e-7,
    //     so planning's band is 99.4% used;
    //   proved  root 0.009_999_4_f32   = 10_736_774 * 2^-30
    //     the rebase multiplier `1.0f32 / 0.01f32` is 100.0 exactly, and
    //     fl32(10_736_774 * 2^-30 * 100) = 1 - 1007 * 2^-24, so the
    //     unit-scale residual is 1007 * 2^-24 = 6.002187728881836e-5
    //     against the 512 * 2^-23 bound, 98.3% used.
    //
    //   divergence = (10_737_525 - 10_736_774) / 10_737_525
    //              = 751 / 10_737_525 = 6.994162993799782e-5
    //   ceiling    = 1e-5 + 2^-14     = 7.103515625e-5
    //   headroom   =                    1.0935263120021840e-6
    let planned = noisy_factor_document(0.010_000_099);
    let proved = noisy_factor_document(0.009_999_4);
    let capability = complete_capability();
    let plan = plan_scale(&noisy_factor_request(&planned, &capability)).unwrap();
    let candidate = build_scale_candidate(&proved, &plan).unwrap();
    let proof = prove_scale(&proved, &candidate, &plan).unwrap();

    assert_eq!(proof.unit_scale.max(), 1007.0 * 2f64.powi(-24));
    assert_eq!(proof.observed_factor_divergence, 751.0 / 10_737_525.0);
    let ceiling = plan.tolerance_policy().observed_factor_divergence_ceiling();
    assert!(
        proof.observed_factor_divergence < ceiling,
        "divergence {} exceeds ceiling {ceiling}",
        proof.observed_factor_divergence
    );
    // Both bands are nearly spent, so the margin left is small — which is
    // what makes this a test of the ceiling rather than of a fixture that
    // never approached it.
    assert!(ceiling - proof.observed_factor_divergence < 1.1e-6);
}

#[test]
fn a_document_whose_skeleton_and_projection_disagree_is_proved_not_refused() {
    // Issue #306's own construction: one document whose normalized
    // skeleton and raw source projection disagree about the scaled root's
    // rest scale. Every other divergence fixture here separates the two
    // witnesses by planning document A and proving document B, which is a
    // property of the pair rather than of a document; this skews
    // `skeleton.bones[0].rest.scale` alone and leaves the projection
    // untouched, so the disagreement is inside the single document both
    // witnesses are read from.
    //
    //   projection root `0.009_999_9_f32`   = 10_737_311 * 2^-30
    //     planning's witness, measured through `parent_source_node_index`.
    //     |s - 0.01| = 9.987503290197208e-8 against the band
    //     1e-5 * max(s, 0.01) = 1e-7, so 99.9% of it is used and planning
    //     accepts.
    //   skeleton root   `0.010_000_611_f32` = 10_738_074 * 2^-30
    //     proof's witness, measured through `world_rest_matrices`, and
    //     also what `build_scale_candidate` rebases. The multiplier
    //     `1.0f32 / 0.01f32` is `100.0` exactly (as in the fixture above),
    //     and fl32(10_738_074 * 2^-30 * 100) = 1 + 512 * 2^-23, so the
    //     unit-scale residual is exactly `2^-14` — the bound itself,
    //     admitted because every policy quantity is an inclusive "at
    //     most".
    //
    //   divergence = (10_738_074 - 10_737_311) / 10_738_074
    //              = 763 / 10_738_074 = 7.105557290813977e-5
    //   ceiling    = 1e-5 + 2^-14     = 7.103515625e-5
    //
    // So the divergence *exceeds* the ceiling while each witness honours
    // its own band — planning's with room to spare, the postcondition's
    // to the last ulp — because the rebase rounds to binary32 on the way
    // and the pre-rounding ratio, 65_576/1_073_741_824 = 6.10723e-5, is
    // itself already past `2^-14`. This is the counterexample that
    // justifies the chosen behaviour: the divergence is *recorded* and the
    // proof succeeds. Refusing at the ceiling would refuse this document.
    let mut doc = noisy_factor_document(0.009_999_9);
    doc.skeleton.bones[0].rest.scale = Vec3::splat(0.010_000_611);
    let capability = complete_capability();
    let plan = plan_scale(&noisy_factor_request(&doc, &capability)).unwrap();
    assert_eq!(plan.observed_factor(), 0.009_999_900_124_967_098);

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.observed_factor, 0.010_000_610_724_091_53);
    assert_eq!(proof.unit_scale.max(), 2f64.powi(-14));
    assert_eq!(proof.observed_factor_divergence, 763.0 / 10_738_074.0);

    let ceiling = plan.tolerance_policy().observed_factor_divergence_ceiling();
    assert!(
        proof.observed_factor_divergence > ceiling,
        "divergence {} did not exceed ceiling {ceiling}",
        proof.observed_factor_divergence
    );
}

#[test]
fn a_whole_document_conversion_reports_one_factor_under_both_names() {
    // That operation declares its factor rather than observing one
    // (§D.1: nothing may infer it from the document), so both witnesses
    // are the declared factor and the divergence is structurally zero.
    // A consumer reading the record does not have to special-case the
    // operation to learn that.
    //
    // The zero here is the weakest of the three assertions — an
    // implementation that reported a constant zero divergence would
    // satisfy it, and the two rest/bind fixtures above are what refuse
    // that. What this one pins is the *factors*: `payload_document`'s rig
    // is unit-scaled throughout, so an implementation that measured a
    // whole-document conversion's observed factor from the document
    // instead of declaring it would report `1.0` under one name, `0.01`
    // under the other, and a divergence of `0.99`.
    let doc = payload_document();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(plan.common_factor(), 0.01);
    assert_eq!(proof.planned_observed_factor, 0.01);
    assert_eq!(proof.observed_factor, 0.01);
    assert_eq!(proof.observed_factor_divergence, 0.0);
}

/// A document whose two parent hierarchies contradict each other, and
/// whose contradiction is a false proof rather than a curiosity.
///
/// `Skeleton::parent` says bone 0 parents bone 1. The source-node
/// projection's `parent_source_node_index` says node 1 (bone 1) parents
/// node 0 (bone 0). Each hierarchy carries its own local rests, and both
/// projected factors sit inside the `1e-5` common-factor band around the
/// declared `0.01`:
///
///   projection root  node 1 -> bone 1  local `0.01_f32`
///   projection child node 0 -> bone 0  local `1 + 2^-18`
///
/// while the skeleton keeps the ordinary shape — bone 0 the root at
/// `0.01`, bone 1 its unit-scaled child.
///
/// Planned at source node 0 the projection makes that node a leaf, so the
/// affected closure is `{0}` and bone 1 is *declared unaffected*. The
/// rebase divides bone 0's local rest by `s` and leaves bone 1's alone —
/// and bone 1 is bone 0's skeleton *child*, so its world rest is
/// multiplied by `1/s`. Both bones move from world scale `0.01` to `1.0`,
/// the skinned instance's vertex at `(0.01, 0, 0)` lands at `(1, 0, 0)`,
/// and every §D.6 residual reports `0.0`: the rest and unit-scale walks
/// iterate `plan.affected_nodes`, [`check_unaffected_instance_binds`]
/// compares effective binds rather than world placement, and
/// [`check_skin_and_bounds`] skips an instance with no affected joint.
///
/// [`crate::model::validate_document_shape`] is what refuses it, which is why
/// this is a refusal exhibit rather than a proving document.
fn contradictory_parent_chain_document() -> Document {
    let mut doc = rig_document(
        &[
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.01),
            },
            rig(Some(0), 1, Vec3::ZERO),
        ],
        &[0],
        0,
        Mat4::IDENTITY,
    );
    let nodes = &mut doc.assets.source_skeleton.nodes;
    nodes[0].parent_source_node_index = Some(1);
    nodes[0].scene_root_indices = Vec::new();
    nodes[0].local_rest = SourceNodeLocalRest::Trs {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(1.0 + 2f32.powi(-18)),
    };
    nodes[1].parent_source_node_index = None;
    nodes[1].scene_root_indices = vec![0];
    nodes[1].local_rest = SourceNodeLocalRest::Trs {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(0.01),
    };
    doc
}

// --- Parent-chain agreement -------------------------------------------
#[test]
fn the_contradictory_parent_chain_documents_false_proof_is_refused() {
    // Planned at source node 0, `contradictory_parent_chain_document`
    // closes over `{0}` and declares bone 1 unaffected — while bone 1 is
    // bone 0's *skeleton* child, so the rebase multiplies its world rest
    // by `1 / s`. Every declared residual is `0.0` for that candidate and
    // the skinned vertex outside the closure moves a hundredfold: the
    // §D.6 record would state that geometry the operation promised not to
    // touch was untouched, and be wrong.
    //
    // Node 0 is the one the projection says is a child of node 1, so it
    // is the node the disagreement is reported at, whichever root the
    // caller asks for.
    let doc = contradictory_parent_chain_document();
    let capability = complete_capability();
    for source_root_node_index in [0, 1] {
        assert_eq!(
            plan_scale(&ScaleRequest {
                operation: ScaleOperation::RestBindUniformScale {
                    source_skin_index: 0,
                    source_root_node_index,
                    expected_factor: 0.01,
                },
                document: &doc,
                capability: &capability,
            })
            .unwrap_err(),
            ScaleError::InvalidDocumentShape(DocumentShapeError::SourceProjection {
                source_node_index: 0,
                violation: SourceProjectionViolation::NearestProjectedParentMismatch,
            })
        );
    }
}

#[test]
fn a_candidate_whose_parent_chains_disagree_is_refused_by_proof() {
    // `build_scale_candidate` and `prove_scale` do not require their two
    // documents to be the same one, so the check has to hold on the
    // candidate as well as the source — a rewritten document whose
    // projection no longer describes its own skeleton is exactly as
    // unprovable as an input one.
    let doc = compensated_document();
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut doctored = candidate.document().clone();
    doctored.assets.source_skeleton.nodes[2].parent_source_node_index = Some(0);
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate { document: doctored }, &plan).unwrap_err(),
        ScaleError::InvalidDocumentShape(DocumentShapeError::SourceProjection {
            source_node_index: 2,
            violation: SourceProjectionViolation::NearestProjectedParentMismatch,
        })
    );
}

#[test]
fn a_projection_that_is_not_complete_coverage_is_not_identity_evidence() {
    // Why the coverage gate is in the check. Under any coverage but
    // `Complete` the projection is the absent-evidence default, so it is
    // not identity evidence about this document — and "not identity
    // evidence" is not the same as "wrong". There is nothing to
    // contradict, so there is no refusal ground.
    //
    // The gate is defensive rather than load-bearing, and this test says
    // so rather than overclaiming. The empty projection every synthesizing
    // producer emits (`static_bake`, `skinned_canonical`, and the FBX
    // loader, which has no source-node table at all) is accepted by the
    // checks themselves whether the gate runs or not — that is
    // `an_empty_projection_under_complete_coverage_is_accepted`, which
    // asserts the same shape with the gate open. The gate's only live
    // behavioural effect
    // is the second half: a document declaring non-`Complete` coverage
    // while carrying a *non-empty* projection that contradicts its
    // skeleton, a shape no in-tree producer emits.
    let capability = complete_capability();
    let mut doc = compensated_document();
    doc.assets.source_skeleton = SourceSkeletonAssets::default();
    assert_eq!(
        doc.assets.source_skeleton.coverage,
        SourceSkeletonCoverage::Unavailable
    );
    assert_eq!(doc.skeleton.bones.len(), 3);
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    })
    .expect("an unavailable projection beside a populated skeleton still plans");

    // And a projection that outright contradicts the skeleton passes too,
    // because under `Unavailable` coverage it is not a claim about this
    // document's identity. Rest/bind planning, which is the operation that
    // would act on it, refuses such a document for the coverage itself.
    let mut contradictory = contradictory_parent_chain_document();
    contradictory.assets.source_skeleton.coverage = SourceSkeletonCoverage::Unavailable;
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &contradictory,
        capability: &capability,
    })
    .expect("an unavailable projection is not checked against the skeleton");
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &contradictory,
            capability: &capability,
        })
        .unwrap_err(),
        ScaleError::IncompleteSourceSkeleton
    );
}

/// A `Complete` projection describing bones 0 and 1 but not bone 2, which
/// is bone 0's *skeleton* child — the second false-proof exhibit, reached
/// from the unprojected side rather than the disagreeing one.
///
/// Planned at source node 0 the closure can only be `{0, 1}`: bone 2 has
/// no source node, so nothing can put it there. The reference rest/bind writer
/// divides bone 0's local rest by `s` and leaves bone 2's alone, so bone
/// 2's world rest is multiplied by `1 / s` — and every obligation looks
/// away for the same three reasons as
/// [`contradictory_parent_chain_document`]. Confirmed through the public
/// API before this closure requirement existed: the operation planned,
/// built and *proved*, with every §D.6 residual `0.0`, while bone 2's
/// world translation moved from `(0.05, 0, 0)` to `(5, 0, 0)` and the
/// skinned vertex hanging off it moved a hundredfold with it.
fn unprojected_skeleton_child_document() -> Document {
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
        // A sibling of the skin's only joint, not a descendant of it: the
        // displacement comes from the shared *skeleton* parent, so it does
        // not need the joint's own subtree.
        rig(Some(0), 2, Vec3::new(5.0, 0.0, 0.0)),
    ];
    let joint_world = Mat4::from_scale_rotation_translation(
        Vec3::splat(0.01),
        Quat::IDENTITY,
        Vec3::new(0.0, 1.0, 0.0),
    );
    let mut doc = rig_document(&nodes, &[1], 0, joint_world.inverse());
    // The geometry that moves. Its only joint is the unprojected bone, so
    // `check_skin_and_bounds` skips it for having no affected joint.
    let outside_world = Mat4::from_scale_rotation_translation(
        Vec3::splat(0.01),
        Quat::IDENTITY,
        Vec3::new(0.05, 0.0, 0.0),
    );
    doc.assets.instances.push(MeshInstance {
        source_node_index: 2,
        node: 2,
        mesh: 0,
        skin_joints: vec![2],
        skin_ibms: vec![outside_world.inverse()],
    });
    doc.assets
        .source_skeleton
        .nodes
        .retain(|node| node.source_node_index != 2);
    doc
}

#[test]
fn a_bone_the_projection_omits_below_one_it_describes_is_refused() {
    // Downward closure, in the direction that must refuse. Reported at
    // source node 0 — the projecting node of the *parent*, since the
    // unprojected child has no source-node index to name.
    let capability = complete_capability();
    let doc = unprojected_skeleton_child_document();
    let expected = ScaleError::InvalidDocumentShape(DocumentShapeError::SourceProjection {
        source_node_index: 0,
        violation: SourceProjectionViolation::ProjectedBoneHasUnprojectedSkeletonChild,
    });
    // A document-shape fact, so every entry point gets it...
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &doc,
            capability: &capability,
        })
        .unwrap_err(),
        expected
    );
    // ...including the rest/bind operation that would otherwise plan,
    // build and prove the false proof this fixture documents.
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap_err(),
        expected
    );
}

#[test]
fn a_bone_with_no_projected_ancestor_is_not_a_disagreement() {
    // Downward closure, in the direction that must *accept* — the reason
    // the rule is closure rather than blunt surjectivity. Re-root the
    // unprojected bone so nothing the projection describes is above it:
    // no closure can contain its parent, because it has none, so no
    // rebase can displace it and there is nothing for the check to refuse.
    // A `claimed.len() == bones.len()` test would refuse this document,
    // and with it every skeleton that legitimately carries more than its
    // projection describes.
    let capability = complete_capability();
    let mut doc = unprojected_skeleton_child_document();
    doc.skeleton.bones[2].parent = None;
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    })
    .expect("an unprojected bone with no projected ancestor is not a disagreement");
}

/// `unprojected_skeleton_child_document`'s displacement reached from the
/// *projected* side: bone 2 keeps its source node, but that node calls
/// itself a projection root while `Skeleton::parent` makes bone 2 a child
/// of bone 0.
///
/// Planned at source node 0 the closure is again `{0, 1}` — node 2 is not
/// a source-node descendant of node 0, so nothing puts it there — while
/// the rebase divides bone 0's local rest by `s` and leaves bone 2's
/// alone. Bone 2's world rest is multiplied by `1 / s` and every
/// obligation looks away for the same three reasons. This is the exhibit
/// for the `None` vs `Some` direction of the parent comparison: with that
/// direction unguarded the document below plans, builds and *proves*, with
/// every §D.6 residual `0.0`, while bone 2's world translation moves from
/// `(0.05, 0, 0)` to `(5, 0, 0)`.
fn projection_root_of_a_skeleton_child_document() -> Document {
    let mut doc = unprojected_skeleton_child_document();
    let mut evidence = SourceNodeAsset::new(
        2,
        SourceNodeLocalRest::Trs {
            translation: Vec3::new(5.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    evidence.bone = Some(2);
    evidence.scene_root_indices = vec![0];
    doc.assets.source_skeleton.nodes.push(evidence);
    doc
}

#[test]
fn a_projection_root_of_a_skeleton_child_is_refused() {
    // The direction M9 unguards. A comparison that fires only when the
    // projected parent is present accepts this document, and accepting it
    // is issue #309 verbatim: planned, built, *proved*, every residual
    // `0.0`, bone 2 displaced a hundredfold.
    let capability = complete_capability();
    let doc = projection_root_of_a_skeleton_child_document();
    let expected = ScaleError::InvalidDocumentShape(DocumentShapeError::SourceProjection {
        source_node_index: 2,
        violation: SourceProjectionViolation::NearestProjectedParentMismatch,
    });
    // A document-shape fact, so every entry point gets it...
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &doc,
            capability: &capability,
        })
        .unwrap_err(),
        expected
    );
    // ...including the rest/bind operation that would otherwise plan,
    // build and prove the false proof this fixture documents.
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap_err(),
        expected
    );
}

#[test]
fn a_container_node_above_the_first_joint_is_not_a_disagreement() {
    // The ordinary glTF export shape, and the one an immediate-parent
    // comparison refuses: an `Armature`/transform node that is not a joint
    // sits above the first joint, so it carries no bone, while the
    // skeleton makes that first joint a root. `bone(parent(n))` has no
    // value, but the projection is not thereby contradicting anything —
    // the node it names never became a bone, so `Bone::parent` could not
    // have named it. The comparison is against the nearest projected
    // ancestor, which here is correctly `None`.
    //
    // Refusing this would refuse both operations on a document that is
    // otherwise legal, including the whole-document conversion, which
    // never reads `bone` at all.
    let capability = complete_capability();
    let mut doc = compensated_document();
    let mut container = SourceNodeAsset::new(7, SourceNodeLocalRest::Matrix(Mat4::IDENTITY));
    container.name = Some("Armature".into());
    container.scene_root_indices = vec![0];
    doc.assets.source_skeleton.nodes.push(container);
    doc.assets.source_skeleton.nodes[0].parent_source_node_index = Some(7);
    doc.assets.source_skeleton.nodes[0].scene_root_indices = Vec::new();
    assert_eq!(doc.skeleton.bones[0].parent, None);

    for operation in [
        ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
    ] {
        let plan = plan_scale(&ScaleRequest {
            operation,
            document: &doc,
            capability: &capability,
        })
        .expect("a container node above the first joint is not a disagreement");
        let candidate = build_scale_candidate(&doc, &plan).expect("the rewrite is buildable");
        prove_scale(&doc, &candidate, &plan).expect("and the rewrite proves");
    }

    // A *chain* of container nodes is the same shape, and a deeper one
    // still terminates: 8 above 7, both unprojected.
    let mut container = SourceNodeAsset::new(8, SourceNodeLocalRest::Matrix(Mat4::IDENTITY));
    container.scene_root_indices = vec![0];
    doc.assets.source_skeleton.nodes.push(container);
    let count = doc.assets.source_skeleton.nodes.len();
    doc.assets.source_skeleton.nodes[count - 2].parent_source_node_index = Some(8);
    doc.assets.source_skeleton.nodes[count - 2].scene_root_indices = Vec::new();
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    })
    .expect("a chain of container nodes above the first joint is not a disagreement either");
}

#[test]
fn an_unprojected_node_between_two_joints_is_not_a_disagreement() {
    // The same step-over, in the middle of the chain rather than above it:
    // node 5 carries no bone and sits between node 1 (bone 1) and node 2
    // (bone 2), while `Skeleton::parent` still makes bone 1 the parent of
    // bone 2. A pruned intermediate or a camera hung between two joints
    // reaches this shape, and the nearest projected ancestor is bone 1
    // either way.
    let capability = complete_capability();
    let mut doc = compensated_document();
    let mut middle = SourceNodeAsset::new(5, SourceNodeLocalRest::Matrix(Mat4::IDENTITY));
    middle.parent_source_node_index = Some(1);
    doc.assets.source_skeleton.nodes.push(middle);
    doc.assets.source_skeleton.nodes[2].parent_source_node_index = Some(5);
    assert_eq!(doc.skeleton.bones[2].parent, Some(1));
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    })
    .expect("an unprojected node between two joints is not a disagreement");
}

#[test]
fn a_source_node_that_never_became_a_bone_is_not_a_disagreement() {
    // Totality is not required of a `Complete` table.
    // `SourceNodeAsset::bone` documents `None` as the loader having
    // dropped that source node during normalization, `SourceNodeAsset::new`
    // starts it absent, and the type is `#[non_exhaustive]` so an embedder
    // assigns only the facts it has. A node that never became a bone
    // cannot be displaced by a rewrite, so agreement is validated over the
    // projected nodes only.
    let capability = complete_capability();
    let mut doc = compensated_document();
    let mut evidence = SourceNodeAsset::new(
        3,
        SourceNodeLocalRest::Trs {
            translation: Vec3::new(7.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    evidence.scene_root_indices = vec![0];
    assert_eq!(evidence.bone, None);
    doc.assets.source_skeleton.nodes.push(evidence);
    for operation in [
        ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
    ] {
        plan_scale(&ScaleRequest {
            operation,
            document: &doc,
            capability: &capability,
        })
        .expect("bare source-node evidence is not a chain disagreement");
    }

    // Inside the rest/bind closure the same node is still refused — but by
    // `SourceNodeNotNormalized`, which is about resolving a selector and
    // not about the two chains disagreeing. Pinned so the boundary between
    // the two is explicit rather than inferred.
    doc.assets.source_skeleton.nodes[3].parent_source_node_index = Some(2);
    doc.assets.source_skeleton.nodes[3].scene_root_indices = Vec::new();
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    })
    .expect("a whole-document conversion never reads `bone` at all");
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap_err(),
        ScaleError::SourceNodeNotNormalized {
            source_node_index: 3
        }
    );

    // And the shape `measure`'s own fixtures declare: `Complete` coverage
    // over a source table where *no* node names a bone, beside a populated
    // skeleton. Nothing is projected, so injectivity and parent
    // preservation quantify over nothing and no bone has a projected
    // parent to hang below. Refusing this is what a totality rule would
    // do, to every one of those fixtures and for every operation.
    let mut all_unprojected = compensated_document();
    for node in &mut all_unprojected.assets.source_skeleton.nodes {
        node.bone = None;
    }
    assert_eq!(all_unprojected.skeleton.bones.len(), 3);
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &all_unprojected,
        capability: &capability,
    })
    .expect("a wholly unprojected complete table beside a populated skeleton still plans");
}

#[test]
fn an_empty_projection_under_complete_coverage_is_accepted() {
    // Refusing this survived as a mutation, so it is pinned directly.
    // `Complete` declares that the source tables describe the loaded
    // input; an empty node table beside a populated skeleton makes no
    // claim the skeleton can contradict. Injectivity and parent
    // preservation quantify over nothing, and downward closure is vacuous
    // because no bone has a projecting node for another to hang below.
    // This is also why the coverage gate cannot be justified as "without
    // it every `SourceSkeletonAssets::default()` document is refused" —
    // that shape is accepted with the gate open.
    let capability = complete_capability();
    let mut doc = compensated_document();
    doc.assets.source_skeleton.nodes.clear();
    assert_eq!(
        doc.assets.source_skeleton.coverage,
        SourceSkeletonCoverage::Complete
    );
    assert_eq!(doc.skeleton.bones.len(), 3);
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    })
    .expect("an empty projection under complete coverage is not a disagreement");
}

#[test]
fn a_complete_projection_beside_an_empty_skeleton_is_accepted() {
    // Skipping the whole check when the skeleton is empty survived as a
    // mutation, so the behaviour it would have shadowed is pinned. This is
    // `measure`'s fixture shape: `Complete` coverage over source-evidence
    // nodes that never became bones, beside a skeleton with no bones at
    // all. Every node is unprojected, so there is nothing to compare —
    // and the check is not merely skipped here, as the second half shows.
    let capability = complete_capability();
    let mut doc = Document::default();
    let mut root = SourceNodeAsset::new(0, SourceNodeLocalRest::Matrix(Mat4::IDENTITY));
    root.scene_root_indices = vec![0];
    let mut child = SourceNodeAsset::new(1, SourceNodeLocalRest::Matrix(Mat4::IDENTITY));
    child.parent_source_node_index = Some(0);
    doc.assets.source_skeleton = SourceSkeletonAssets {
        coverage: SourceSkeletonCoverage::Complete,
        nodes: vec![root, child],
        skins: Vec::new(),
    };
    assert!(doc.skeleton.bones.is_empty());
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    })
    .expect("unprojected source-node evidence beside an empty skeleton still plans");

    // A node that *does* name a bone is still checked against that empty
    // skeleton, so the acceptance above is emptiness of the projection
    // rather than the check declining to run.
    doc.assets.source_skeleton.nodes[1].bone = Some(0);
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &doc,
            capability: &capability,
        })
        .unwrap_err(),
        ScaleError::InvalidDocumentShape(DocumentShapeError::SourceProjection {
            source_node_index: 1,
            violation: SourceProjectionViolation::ProjectedBoneOutOfRange,
        })
    );
}

/// `noisy_factor_document`'s rig with a per-axis root rest scale.
///
/// Only ever a *proof* source: [`prove_scale`] deliberately does not
/// re-run [`classify_affine`], so nothing on that path bounds how far
/// apart the scaled root's three axes are, and the test below asserts
/// that planning this same document is refused.
fn per_axis_factor_document(scale: Vec3) -> Document {
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale,
        },
        rig(Some(0), 1, Vec3::ZERO),
    ];
    rig_document(&nodes, &[1], 0, Mat4::IDENTITY)
}

#[test]
fn the_observed_factor_is_the_mean_of_the_scaled_roots_axes_not_its_first_one() {
    // `observed_factor_from_source` reports
    // `average_affine_axis_length(affine_axis_lengths(..))` — the same pair of helpers
    // `classify_affine` returns its factor through, which is what makes
    // the two "the same quantity by definition rather than by
    // coincidence". On a basis whose axes are equal within
    // `equal_axis`, though, the mean and any single axis agree to within
    // that band, and every other proof-source fixture here has a
    // *uniform* scaled root — so nothing separates the mean from, say,
    // its first column.
    //
    // Something has to. Proof does not re-classify its source, by design,
    // so the axis spread of the document `prove_scale` is handed is
    // bounded by nothing at all: the only downstream constraint is the
    // unit-scale postcondition on the rebased *candidate*, which admits
    // `postcondition_unit_scale_residual = 2^-14 = 6.10e-5` of it — six
    // times the `1e-5` equal-axis band planning would have applied. The
    // fixture uses half of what the postcondition allows.
    //
    // The fixture puts the root's axes a fixed `328` ulps either side of
    // `0.01_f32`, which is `10_737_418 * 2^-30`:
    //
    //   x  (10_737_418 + 328) * 2^-30      y  (10_737_418 - 328) * 2^-30
    //   z   10_737_418        * 2^-30
    //
    // Each is exact in binary32 (`10_737_746 < 2^24`), so each column
    // length is exact, and their sum telescopes to `3 * 10_737_418 *
    // 2^-30` — 26 bits, exact in binary64 — whose third is `0.01_f32`
    // again. The mean is therefore exactly `NEAR_UNIT_OBSERVED_FACTOR`
    // while no individual axis is, and `328 / 10_737_418 = 3.05e-5` of
    // spread stays inside the postcondition band the rebase must meet.
    let step = 328.0 * 2f32.powi(-30);
    let proved = per_axis_factor_document(Vec3::new(0.01 + step, 0.01 - step, 0.01));
    let root = proved.skeleton.bones[0].rest.scale;
    let ulps = 328.0 * 2f64.powi(-30);
    assert_eq!(f64::from(root.x), NEAR_UNIT_OBSERVED_FACTOR + ulps);
    assert_eq!(f64::from(root.y), NEAR_UNIT_OBSERVED_FACTOR - ulps);
    assert_eq!(f64::from(root.z), NEAR_UNIT_OBSERVED_FACTOR);

    // Planning refuses it — `3.05e-5` of spread is three times the
    // `equal_axis` band — which is exactly why the spread has to arrive
    // through the proof source rather than through a plan.
    let capability = complete_capability();
    assert_eq!(
        plan_scale(&noisy_factor_request(&proved, &capability)).unwrap_err(),
        ScaleError::InvalidAffineDomain {
            node: 0,
            reason: AffineDomainViolation::NonUniformScale,
        }
    );

    let planned = noisy_factor_document(0.01);
    let plan = plan_scale(&noisy_factor_request(&planned, &capability)).unwrap();
    let candidate = build_scale_candidate(&proved, &plan).unwrap();
    let proof = prove_scale(&proved, &candidate, &plan).unwrap();
    assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);

    // The rebase multiplier is the exact `1.0_f32 / 0.01_f32 == 100`, so
    // the candidate's root axes are `fl32(x * 100)` and `fl32(y * 100)`:
    // `1_073_774_600 * 2^-30` rounds to `1 + 2^-15`, and `1_073_709_000 *
    // 2^-30` rounds to `(2^24 - 513) * 2^-24`. The larger deviation from
    // one is the second, `513 * 2^-24`, and the postcondition admits it
    // with room to spare.
    assert_eq!(proof.unit_scale.max(), 513.0 * 2f64.powi(-24));
    assert!(proof.unit_scale.max() <= plan.tolerance_policy().postcondition_unit_scale_residual);
}

#[test]
fn observed_factor_from_source_uses_the_canonical_appendix_d_v6_mean() {
    // Planning correctly refuses these association-sensitive inputs, but
    // proof deliberately does not re-classify its source. Drive the
    // production proof helper directly with every proper permutation so
    // an authored-column-order sum cannot survive solely on that path.
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document: &doc,
        capability: &capability,
    })
    .expect("the unit document supplies a valid rest/bind plan");
    let expected = 0x3ff1_09ef_b555_6f3f;

    for (permutation, linear) in
        crate::model::affine_test_fixtures::appendix_d_v6_mean_permutations()
            .into_iter()
            .enumerate()
    {
        let source_worlds = WorldPose {
            bones: vec![WorldBonePose {
                matrix: Mat4::from_cols(
                    linear.x_axis.extend(0.0),
                    linear.y_axis.extend(0.0),
                    linear.z_axis.extend(0.0),
                    Vec4::W,
                ),
                translation_rounding_magnitude: 0.0,
            }],
        };
        assert_eq!(
            observed_factor_from_source(&doc, &source_worlds, &plan)
                .unwrap()
                .to_bits(),
            expected,
            "orientation-preserving permutation {permutation}"
        );
    }
}

#[test]
fn a_whole_document_plan_reports_its_declared_factor_as_the_observed_one() {
    // §D.1: a whole-document conversion's factor is declared, never
    // inferred from the document. Two documents authored in different
    // linear units are numerically identical, so there is nothing to
    // measure and the declared factor is the whole of the evidence.
    //
    // The fixture makes that a real assertion rather than a tautology:
    // `unit_rig`'s root has rest scale one, so an implementation that
    // measured the rest-world factor here — as the rest/bind operation
    // does — would report `1.0`, not the declared `0.01`.
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    assert_eq!(plan.common_factor(), 0.01);
    assert_eq!(plan.observed_factor(), 0.01);

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.observed_factor, 0.01);
}

#[test]
fn a_rest_bind_plan_cannot_be_replayed_against_a_renumbered_selector_domain() {
    // The observed factor is re-derived through the proof source's own
    // source-node projection. A source that has renumbered the selected
    // root is not the structural domain the plan describes and is now
    // rejected while re-planning, before proof could report a measurement
    // for the wrong domain.
    let doc = compensated_document();
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // Renumbering rather than deleting keeps the document internally
    // consistent — same tree and same bones, different source identity —
    // so document-shape validation itself still accepts it.
    let mut unprojected = doc.clone();
    unprojected.assets.source_skeleton.nodes[0].source_node_index = 7;
    unprojected.assets.source_skeleton.nodes[1].parent_source_node_index = Some(7);
    let mut unprojected_candidate = candidate.into_document();
    unprojected_candidate.assets.source_skeleton.nodes[0].source_node_index = 7;
    unprojected_candidate.assets.source_skeleton.nodes[1].parent_source_node_index = Some(7);
    assert_eq!(
        prove_scale(
            &unprojected,
            &ScaleCandidate {
                document: unprojected_candidate
            },
            &plan
        )
        .unwrap_err(),
        ScaleError::InvalidRootSelector {
            source_root_node_index: 0,
        }
    );
}

#[test]
fn plan_scale_accepting_a_common_factor_implies_its_candidate_proves() {
    // The closure property DESIGN.md Appendix D §D.1 now states outright:
    // a factor `plan_scale` accepts always yields a candidate that
    // satisfies the unit-scale postcondition. Before the v2 policy the
    // implication was false over a whole band, because the input check
    // was a scalar relative comparison and the postcondition was an L2
    // norm over three axes: every relative error `e` in `(5.77e-6, 1e-5]`
    // was accepted and then rejected at `sqrt(3) * e > 1e-5`.
    //
    // Each fixture is a binary32 value chosen so `fl(s * 100)` lands
    // exactly on a mantissa grid point `1 + n * 2^-23`, which is exactly
    // what the rebased root's composed scale becomes (`1.0f32 / 0.01f32`
    // is `100.0` exactly). The expected residual is therefore the literal
    // `n * 2^-23`, hand-computable and independent of this module:
    //
    //   s                   exact value of s              n   n * 2^-23
    //   0.010_000_02        0.0100000202655792236328125   17  2.02655792236328125e-6
    //   0.010_000_061       0.0100000612437725067138671875 51 6.07967376708984375e-6
    //   0.010_000_08        0.0100000798702239990234375   67  7.98702239990234375e-6
    //   0.010_000_099       0.010000099427998065948486328125
    //                                                     83  9.89437103271484375e-6
    //
    // The last three all sit in the previously-broken band: under the v1
    // L2 residual they measured 1.053e-5, 1.383e-5 and 1.714e-5 against a
    // `1e-5` bound and failed proof outright.
    let capability = complete_capability();
    for (scale, expected_residual) in [
        (0.010_000_02_f32, 17.0 * 2f64.powi(-23)),
        (0.010_000_061_f32, 51.0 * 2f64.powi(-23)),
        (0.010_000_08_f32, 67.0 * 2f64.powi(-23)),
        (0.010_000_099_f32, 83.0 * 2f64.powi(-23)),
    ] {
        let doc = noisy_factor_document(scale);
        let request = noisy_factor_request(&doc, &capability);
        let plan = plan_scale(&request)
            .unwrap_or_else(|error| panic!("scale {scale} must plan, got {error:?}"));
        let candidate = build_scale_candidate(&doc, &plan)
            .unwrap_or_else(|error| panic!("scale {scale} must build, got {error:?}"));
        let proof = prove_scale(&doc, &candidate, &plan)
            .unwrap_or_else(|error| panic!("scale {scale} must prove, got {error:?}"));
        assert_eq!(
            proof.unit_scale.max(),
            expected_residual,
            "scale {scale} unit-scale residual"
        );
        assert!(
            proof.unit_scale.max() <= plan.tolerance_policy().postcondition_unit_scale_residual,
            "scale {scale} residual {} exceeds the declared bound",
            proof.unit_scale.max()
        );
    }
}

#[test]
fn the_common_factor_band_stays_relative_below_the_scalar_absolute_floor() {
    // The band's comparison base carries no floor at all. An earlier
    // revision floored it at `scalar_absolute = 1e-6`, which is the same
    // defect as the `1.0` floor above, one thousandth the size: below
    // `1e-6` the band stopped tracking its operands and froze at the
    // constant `1e-5 * 1e-6 = 1e-11`, a *relative* band of `1e-11 / s`
    // that widens without limit as `s` shrinks and crosses the `2^-14`
    // postcondition bound at `s = 1e-11 / 2^-14 = 1e-11 * 16384 =
    // 1.6384e-7`. Every declared factor below that had a band of plans
    // `plan_scale` accepted and `prove_scale` then refused — the closure
    // property §D.1 states as a theorem, false over a whole regime.
    // (Measured with the floor restored: the largest declared factor that
    // still breaks closure is `1.629e-7` and the smallest clean one is
    // `1.710e-7`, bracketing `1.6384e-7`.)
    //
    // `2^-23 = 1.1920928955078125e-7` is well inside that regime and is
    // exactly representable, as is its reciprocal `2^23`, so every step
    // below is exact in binary32:
    //
    //   s          = 2^-23 * (1 + n * 2^-23)
    //   candidate root local scale
    //              = s * (1 / 2^-23) = 1 + n * 2^-23
    //   residual   = n * 2^-23
    //
    // and the band accepts exactly while `n * 2^-23 <= 1e-5 * (1 + n *
    // 2^-23)`, i.e. `n <= 83`:
    //
    //   n = 83: 9.89437103271484375e-6 <= 1.0000098944e-5  -> accept
    //   n = 84: 1.001358032226562e-5   >  1.0000100136e-5  -> reject
    //
    // That is the *same* boundary the `0.01` fixtures above hit, six
    // orders of magnitude away, which is what "relative" means. Under the
    // `1e-6` floor the band here admitted every `n` up to 703, and
    // `n = 600` — the third row below — planned, built, and then failed
    // its own postcondition at `600 * 2^-23 = 7.152557373046875e-5`.
    let declared = 2f64.powi(-23);
    let unit = 2f32.powi(-23);
    let capability = complete_capability();
    for n in [83u32, 84, 600] {
        let doc = noisy_factor_document(unit * (1.0 + n as f32 * unit));
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: declared,
            },
            document: &doc,
            capability: &capability,
        };
        if n > 83 {
            assert!(
                matches!(plan_scale(&request), Err(ScaleError::FactorMismatch { .. })),
                "n = {n} must not be accepted"
            );
            continue;
        }
        let plan =
            plan_scale(&request).unwrap_or_else(|error| panic!("n = {n} must plan, got {error:?}"));
        let candidate = build_scale_candidate(&doc, &plan)
            .unwrap_or_else(|error| panic!("n = {n} must build, got {error:?}"));
        let proof = prove_scale(&doc, &candidate, &plan)
            .unwrap_or_else(|error| panic!("n = {n} must prove, got {error:?}"));
        assert_eq!(proof.unit_scale.max(), f64::from(n) * 2f64.powi(-23));
    }
}

#[test]
fn a_plan_loading_all_three_analytic_bands_still_proves() {
    // Every other closure fixture is a single-node rig, which can only
    // load the `FactorMismatch` band. This one loads all three bands the
    // §D.1 derivation composes, at 76.3% of the declared `1e-5` each:
    //
    //   u = 2^-17 = 7.62939453125e-6
    //   root local scale  = 0.5 * (1 + u)      -> s_0 = 0.5 * (1 + u)
    //   child local scale = (1 + u/2, 1 + u/2, 1 + 2u)
    //
    // Every product below is exact in binary32 (the discarded terms are
    // `2^-36` and `2^-34` against a `2^-24` ulp), so the child's world
    // axis lengths are
    //
    //   x = y = 0.5 * (1 + 1.5u)      z = 0.5 * (1 + 3u)
    //   average A = (2x + z) / 3 = 0.5 * (1 + 2u)
    //
    // and the three bands are:
    //
    //   FactorMismatch     |s_0 - 0.5| = 0.5u    vs 1e-5 * 0.5(1 + u)
    //   MixedFactor        |A - s_0|   = 0.5u    vs 1e-5 * 0.5(1 + 2u)
    //   NonUniformScale    |z - A|     = 0.5u    vs 1e-5 * 0.5(1 + 3u)
    //
    // all of which accept, since `u = 7.63e-6 < 1e-5`. The third band is
    // the one the two-band derivation missed: `classify_affine` returns
    // the *average* of the three axis lengths, while the postcondition
    // measures an individual *axis*, and `equal_axis` permits each axis
    // its own further band away from that average.
    //
    // The candidate's root local scale is `0.5 * (1 + u) * 2 = 1 + u`
    // exactly, so its composed axes are `2x`, `2x`, `2z`, and with
    // `u = 64 * 2^-23`:
    //
    //   node 0 residual = u    =  64 * 2^-23
    //   node 1 residual = 3u   = 192 * 2^-23 = 2.288818359375e-5
    //
    // `192 * 2^-23` is above `(1 - c)^-2 - 1 = 2.00003e-5`, so this rig
    // is a *counterexample* to the two-band worst case the policy used to
    // claim — it is not reachable by composing only the declared-factor
    // and mixed-factor bands.
    let u = 2f32.powi(-17);
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.5 * (1.0 + u)),
        },
        RigNode {
            parent: Some(0),
            source_node_index: 1,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::new(1.0 + u * 0.5, 1.0 + u * 0.5, 1.0 + u * 2.0),
        },
    ];
    let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.5,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.unit_scale.max(), 192.0 * 2f64.powi(-23));
    let policy = plan.tolerance_policy();
    assert!(proof.unit_scale.max() <= policy.postcondition_unit_scale_residual);
    let two_bands = (1.0 - policy.common_factor).powi(-2) - 1.0;
    assert!(
        proof.unit_scale.max() > two_bands,
        "residual {} does not exceed the two-band figure {two_bands}",
        proof.unit_scale.max()
    );
}

#[test]
fn equal_axis_uniformity_is_relative_to_the_authored_magnitude() {
    // `(0.01, 0.01, 0.010005)` is `5e-4` relative non-uniform — 50x the
    // declared `1e-5` equal-axis tolerance — and must classify as
    // non-uniform. A comparison base floored at `1.0` would instead read
    // the `5e-6` absolute spread as uniform.
    let error = reject_case(|rest| *rest = trs_scale(Vec3::new(0.01, 0.01, 0.010005)));
    assert!(
        matches!(
            error,
            ScaleError::InvalidAffineDomain {
                reason: AffineDomainViolation::NonUniformScale,
                ..
            }
        ),
        "unexpected error {error:?}"
    );
}

#[test]
fn a_tiny_expected_factor_does_not_pass_the_common_factor_check_by_absolute_luck() {
    // Source factor `1e-6` against a declared `1e-30`: the two differ by
    // 24 orders of magnitude, which a comparison base floored at `1.0`
    // read as an absolute difference of `1e-6` and accepted.
    let doc = noisy_factor_document(1e-6);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1e-30,
        },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&request).unwrap_err(),
        ScaleError::FactorMismatch { .. }
    ));
}

// --- Closure and selector rejections --------------------------------

#[test]
fn incomplete_capability_rejects_before_geometry_is_inspected() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = ScaleCapabilityFacts::default();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.0 },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&request).unwrap_err(),
        ScaleError::IncompleteCapability
    ));
}

#[test]
fn incomplete_source_skeleton_coverage_rejects() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.assets.source_skeleton.coverage = SourceSkeletonCoverage::Unavailable;
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&request).unwrap_err(),
        ScaleError::IncompleteSourceSkeleton
    ));
}

#[test]
fn invalid_factor_rejects() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    for factor in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::InvalidFactor { .. }
        ));
    }
}

#[test]
fn invalid_source_selectors_reject_without_panicking() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let bad_root = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 99,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&bad_root).unwrap_err(),
        ScaleError::InvalidRootSelector {
            source_root_node_index: 99
        }
    ));
    let bad_skin = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 99,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&bad_skin).unwrap_err(),
        ScaleError::InvalidSkinSelector {
            source_skin_index: 99
        }
    ));
}

#[test]
fn incomplete_closure_when_a_skin_joint_is_outside_the_scaled_roots_descendants() {
    // Root and an unrelated sibling subtree; the skin's joint (bone 2)
    // is not a descendant of the declared root (bone 0).
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(None, 1, Vec3::ZERO),
        rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
    ];
    let doc = rig_document(&nodes, &[2], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document: &doc,
        capability: &capability,
    };
    assert_eq!(
        plan_scale(&request).unwrap_err(),
        ScaleError::IncompleteClosure {
            reason: "joint_not_descendant_of_scaled_root"
        }
    );
}

#[test]
fn descendant_unskinned_geometry_inside_the_closure_rejects() {
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        rig(Some(1), 2, Vec3::new(1.0, 0.0, 0.0)),
    ];
    let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    // An extra unskinned mesh instance attached at bone 2, a descendant
    // of the affected joint.
    doc.assets.meshes.push(MeshAsset {
        name: "prop".into(),
        source_mesh_index: 1,
        primitives: vec![Primitive {
            positions: vec![Vec3::ZERO],
            joints: vec![[0, 0, 0, 0]],
            weights: vec![[1.0, 0.0, 0.0, 0.0]],
            ..Primitive::default()
        }],
    });
    doc.assets.instances.push(MeshInstance {
        source_node_index: 2,
        node: 2,
        mesh: 1,
        skin_joints: Vec::new(),
        skin_ibms: Vec::new(),
    });
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&request).unwrap_err(),
        ScaleError::UnsupportedUnskinnedGeometry { node: 2 }
    ));
}

#[test]
fn root_attached_unskinned_geometry_rejects() {
    // Root (bone 0) and joint (bone 1); an unskinned mesh instance is
    // attached directly at the selected root itself, not a later
    // descendant, so it must still be rejected.
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.assets.meshes.push(MeshAsset {
        name: "prop".into(),
        source_mesh_index: 1,
        primitives: vec![Primitive {
            positions: vec![Vec3::ZERO],
            joints: vec![[0, 0, 0, 0]],
            weights: vec![[1.0, 0.0, 0.0, 0.0]],
            ..Primitive::default()
        }],
    });
    doc.assets.instances.push(MeshInstance {
        source_node_index: 0,
        node: 0,
        mesh: 1,
        skin_joints: Vec::new(),
        skin_ibms: Vec::new(),
    });
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&request).unwrap_err(),
        ScaleError::UnsupportedUnskinnedGeometry { node: 0 }
    ));
}

#[test]
fn ancestor_path_attached_unskinned_geometry_rejects() {
    // Root (bone 0) -> mid (bone 1) -> joint (bone 2); bone 1 is not the
    // root and not a skin joint, it is only reached by walking the
    // joint's ancestor chain, so it must still be rejected.
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
    ];
    let mut doc = rig_document(&nodes, &[2], 0, Mat4::IDENTITY);
    doc.assets.meshes.push(MeshAsset {
        name: "prop".into(),
        source_mesh_index: 1,
        primitives: vec![Primitive {
            positions: vec![Vec3::ZERO],
            joints: vec![[0, 0, 0, 0]],
            weights: vec![[1.0, 0.0, 0.0, 0.0]],
            ..Primitive::default()
        }],
    });
    doc.assets.instances.push(MeshInstance {
        source_node_index: 1,
        node: 1,
        mesh: 1,
        skin_joints: Vec::new(),
        skin_ibms: Vec::new(),
    });
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&request).unwrap_err(),
        ScaleError::UnsupportedUnskinnedGeometry { node: 1 }
    ));
}

#[test]
fn dangling_ancestor_source_parent_index_rejects_without_panicking() {
    // Root (bone 0) -> mid (bone 1) -> joint (bone 2); the source
    // skeleton then drops the projection for bone 1 entirely, leaving
    // bone 2's `parent_source_node_index` dangling. Walking the joint's
    // ancestor chain must fail closed with a typed error rather than
    // panic on an unchecked map index.
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
    ];
    let mut doc = rig_document(&nodes, &[2], 0, Mat4::IDENTITY);
    doc.assets
        .source_skeleton
        .nodes
        .retain(|node| node.source_node_index != 1);
    assert_eq!(
        closure_reject_reason(&doc, 0),
        ScaleError::IncompleteClosure {
            reason: "dangling_source_parent_node_index"
        }
    );
    // And the public path never gets there: bone 2's projection names a
    // parent the projection itself no longer carries.
    assert_eq!(
        rest_bind_reject_reason(&doc),
        ScaleError::InvalidDocumentShape(DocumentShapeError::SourceProjection {
            source_node_index: 2,
            violation: SourceProjectionViolation::ParentSourceNodeMissing,
        })
    );
}

// --- Scale-animation rebase -----------------------------------------

#[test]
fn rest_bind_rebases_root_scale_animation_values_and_cubic_tangents() {
    let mut doc = compensated_document();
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            // The selected closure root. Scale animation replaces rest
            // scale, so identity becomes 1 / 0.01 = 100 rather than
            // being retained as a dimensionless value.
            bone: 0,
            property: Property::Scale,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![
                Vec3::new(-2.0, 3.0, -4.0),    // in tangent @ 0
                Vec3::ONE,                     // value @ 0
                Vec3::new(5.0, -6.0, 7.0),     // out tangent @ 0
                Vec3::new(-8.0, 9.0, -10.0),   // in tangent @ 1
                Vec3::new(11.0, -12.0, 13.0),  // value @ 1
                Vec3::new(-14.0, 15.0, -16.0), // out tangent @ 1
            ]),
        }],
    });
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    assert!(plan.field_rows().iter().any(|row| matches!(
        (row.target, row.disposition),
        (
            ScaleFieldTarget::AnimationValues {
                property: Property::Scale,
                ..
            },
            ScaleFieldDisposition::Rewrite(ScaleRewriteRule::RestBindLocalScale)
        )
    )));
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let TrackValues::Vec3s(values) = &candidate.document().clips[0].tracks[0].values else {
        panic!("expected vec3 scale track");
    };
    let expected = [
        Vec3::new(-200.0, 300.0, -400.0),
        Vec3::splat(100.0),
        Vec3::new(500.0, -600.0, 700.0),
        Vec3::new(-800.0, 900.0, -1000.0),
        Vec3::new(1100.0, -1200.0, 1300.0),
        Vec3::new(-1400.0, 1500.0, -1600.0),
    ];
    assert_eq!(values, &expected);
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn rest_bind_rebases_every_constant_identity_root_scale_key() {
    let mut doc = compensated_document();
    doc.clips.push(Clip {
        name: "constant identity".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Scale,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::ONE]),
        }],
    });
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let TrackValues::Vec3s(values) = &candidate.document().clips[0].tracks[0].values else {
        panic!("expected vec3 scale track");
    };
    assert_eq!(values, &[Vec3::splat(100.0), Vec3::splat(100.0)]);
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn rest_bind_preserves_strict_descendant_and_unaffected_scale_tracks() {
    let mut doc = parent_boundary_document();
    doc.clips.push(Clip {
        name: "scale".into(),
        duration_s: 1.0,
        tracks: vec![
            Track {
                // The selected root itself is rebased by 1 / s.
                bone: 1,
                property: Property::Scale,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::ONE]),
            },
            Track {
                // Its parent is affected, so s_parent / s_i is one.
                bone: 2,
                property: Property::Scale,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::new(2.0, 3.0, 4.0)]),
            },
            Track {
                // The boundary parent is outside the closure.
                bone: 0,
                property: Property::Scale,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::new(5.0, 6.0, 7.0)]),
            },
        ],
    });
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let tracks = &candidate.document().clips[1].tracks;
    let TrackValues::Vec3s(root) = &tracks[0].values else {
        panic!("expected vec3 root scale track");
    };
    let TrackValues::Vec3s(descendant) = &tracks[1].values else {
        panic!("expected vec3 descendant scale track");
    };
    let TrackValues::Vec3s(unaffected) = &tracks[2].values else {
        panic!("expected vec3 unaffected scale track");
    };
    assert_eq!(root, &[Vec3::splat(100.0)]);
    assert_eq!(descendant, &[Vec3::new(2.0, 3.0, 4.0)]);
    assert_eq!(unaffected, &[Vec3::new(5.0, 6.0, 7.0)]);
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn rest_bind_scale_animation_uses_f64_reciprocal_then_one_f32_narrowing() {
    let mut nodes = compensated_rig();
    nodes[0].scale = Vec3::splat(0.03);
    let child_world = Mat4::from_scale_rotation_translation(
        nodes[0].scale,
        nodes[1].rotation,
        Vec3::new(0.0, 3.0, 0.0),
    );
    let mut doc = rig_document(&nodes, &[1], 0, child_world.inverse());
    let original = Vec3::new(0.7, -1.3, 2.9);
    doc.clips.push(Clip {
        name: "scale".into(),
        duration_s: 0.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Scale,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![original]),
        }],
    });
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.03,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let expected_multiplier = 1.0 / plan.common_factor();
    assert_eq!(expected_multiplier, 1.0f64 / 0.03f64);
    assert_ne!(
        expected_multiplier,
        f64::from(1.0f32 / 0.03f32),
        "the stored rewrite must not round the reciprocal through f32"
    );
    let TrackValues::Vec3s(values) = &candidate.document().clips[0].tracks[0].values else {
        panic!("expected vec3 scale track");
    };
    assert_eq!(
        values[0],
        (original.as_dvec3() * expected_multiplier).as_vec3()
    );
    // Public proof is the oracle. Its source-selector derivation is
    // independent of the builder multiplier above.
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn proof_names_a_root_scale_track_left_at_its_source_value() {
    let mut doc = compensated_document();
    doc.clips.push(Clip {
        name: "scale".into(),
        duration_s: 0.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Scale,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![Vec3::ONE]),
        }],
    });
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    // Simulate a builder that failed to rewrite the closure-root scale
    // track. The direct proof expectation must refuse it before sampled
    // obligations can hide the error behind another residual kind.
    let mut no_root_rewrite = candidate.into_document();
    no_root_rewrite.clips[0].tracks[0].values = TrackValues::Vec3s(vec![Vec3::ONE]);
    assert!(matches!(
        prove_scale(&doc, &ScaleCandidate::from_document(no_root_rewrite), &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::TrackValue,
            ..
        }
    ));
}

// --- Stale-plan inventory -------------------------------------------

#[test]
fn build_scale_candidate_rejects_a_scale_track_added_after_planning_without_mutating_the_document()
{
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    // Plan against the clean document through the public API.
    let plan = plan_scale(&request).unwrap();

    // Mutate a *different* document after planning. The added affected
    // track creates sample evidence, so the replayed plan inventory must
    // reject it before a builder can omit that proof work.
    let mut mutated = doc.clone();
    mutated.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Scale,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![Vec3::ONE]),
        }],
    });
    let original_translation = mutated.skeleton.bones[0].rest.translation;
    let original_clip_count = mutated.clips.len();

    let error = build_scale_candidate(&mutated, &plan).unwrap_err();
    assert_eq!(
        error,
        ScaleError::PlanDocumentMismatch {
            reason: "proof_obligations_mismatch"
        }
    );
    assert_eq!(
        mutated.skeleton.bones[0].rest.translation,
        original_translation
    );
    assert_eq!(mutated.clips.len(), original_clip_count);
    assert_eq!(doc.skeleton.bones[0].rest.translation, Vec3::ZERO);
}

#[test]
fn every_rejection_path_leaves_the_source_document_unchanged() {
    let cases: Vec<Box<dyn Fn() -> (Document, ScaleOperation)>> = vec![
        Box::new(|| {
            (
                rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY),
                ScaleOperation::WholeDocumentLinearUnits { factor: -1.0 },
            )
        }),
        Box::new(|| {
            (
                rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY),
                ScaleOperation::RestBindUniformScale {
                    source_skin_index: 0,
                    source_root_node_index: 0,
                    expected_factor: 0.5,
                },
            )
        }),
        Box::new(|| {
            (
                rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY),
                ScaleOperation::RestBindUniformScale {
                    source_skin_index: 99,
                    source_root_node_index: 0,
                    expected_factor: 1.0,
                },
            )
        }),
    ];
    for case in cases {
        let (doc, operation) = case();
        let before = doc.skeleton.bones[0].rest.translation;
        let capability = complete_capability();
        let request = ScaleRequest {
            operation,
            document: &doc,
            capability: &capability,
        };
        assert!(plan_scale(&request).is_err());
        assert_eq!(doc.skeleton.bones[0].rest.translation, before);
    }
}

// --- Duplicate source-skeleton identity (hardening gap 1) -----------

#[test]
fn duplicate_source_node_index_rejects_instead_of_last_write_wins() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    // Two source nodes both claim `source_node_index == 1`; a
    // `BTreeMap`-keyed projection would silently keep only one.
    let mut duplicate = doc.assets.source_skeleton.nodes[1].clone();
    duplicate.source_node_index = 1;
    doc.assets.source_skeleton.nodes.push(duplicate);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&request).unwrap_err(),
        ScaleError::InvalidDocumentShape(DocumentShapeError::DuplicateSourceNodeIndex {
            source_node_index: 1
        })
    ));
}
// --- world_at_time hardening (hardening gap 3) -----------------------

#[test]
fn out_of_range_track_bone_added_after_planning_rejects_without_panicking() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let mut mutated = doc.clone();
    mutated.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 99,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO]),
        }],
    });
    assert!(matches!(
        build_scale_candidate(&mutated, &plan).unwrap_err(),
        ScaleError::InvalidDocumentShape(DocumentShapeError::TrackShape { .. })
    ));
}
// --- skinned_bounds hardening (hardening gap 4) -----------------------

#[test]
fn joint_influence_slot_outside_skin_joints_rejects_without_panicking() {
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut malformed = doc.clone();
    // Skin has one joint (slot 0), but this vertex claims influence
    // slot 5.
    malformed.assets.meshes[0].primitives[0].joints[0] = [5, 0, 0, 0];
    assert!(matches!(
        prove_scale(&malformed, &candidate, &plan).unwrap_err(),
        ScaleError::InvalidSkinnedPrimitive {
            reason: "joint_influence_slot_out_of_range",
            ..
        }
    ));
}

#[test]
fn missing_per_vertex_joints_or_weights_in_a_skinned_primitive_rejects() {
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut malformed = doc.clone();
    // One position, but the weights array is empty: no longer parallel
    // to `positions`.
    malformed.assets.meshes[0].primitives[0].weights.clear();
    assert_eq!(
        prove_scale(&malformed, &candidate, &plan).unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        }
    );
}

#[test]
fn non_finite_vertex_position_in_a_skinned_primitive_rejects() {
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut malformed = doc.clone();
    malformed.assets.meshes[0].primitives[0].positions[0] = Vec3::new(f32::NAN, 0.0, 0.0);
    // Base `POSITION` is a rewritten domain, so its finiteness is now a
    // scale-input invariant checked at every entry point rather than
    // something only the skinned-bounds walk happens to notice — which
    // is what makes it hold for the *candidate* a build returns, too.
    assert!(matches!(
        prove_scale(&malformed, &candidate, &plan).unwrap_err(),
        ScaleError::InvalidMeshPrimitive {
            reason: "non_finite_position",
            ..
        }
    ));
}

#[test]
fn build_validates_the_candidate_it_generated_before_returning_it() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.assets.meshes[0].primitives[0].positions[0] = Vec3::splat(f32::MAX);
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
        document: &doc,
        capability: &capability,
    })
    .expect("the finite source and representable factor plan");

    assert!(matches!(
        build_scale_candidate(&doc, &plan).unwrap_err(),
        ScaleError::InvalidMeshPrimitive {
            reason: "non_finite_position",
            ..
        }
    ));
}

#[test]
fn build_runs_the_shared_shape_checker_on_the_candidate_it_generated() {
    let document = Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform {
                    translation: Vec3::splat(f32::MAX),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            }],
        },
        ..Document::default()
    };
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
        document: &document,
        capability: &capability,
    })
    .expect("the source shape and narrowed factor are individually finite");

    assert_eq!(
        build_scale_candidate(&document, &plan).unwrap_err(),
        ScaleError::InvalidDocumentShape(DocumentShapeError::NonFiniteSkeletonRest { node: 0 })
    );
}

#[test]
fn missing_inverse_bind_evidence_rejects_instead_of_defaulting_to_identity() {
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut malformed = doc.clone();
    // Empty `skin_ibms` falls back to the joint bone's own
    // `inverse_bind`, which is `None` for every bone this fixture
    // builds: there is genuinely no inverse-bind evidence, so this must
    // reject rather than silently substitute identity.
    malformed.assets.instances[0].skin_ibms.clear();
    assert_eq!(
        prove_scale(&malformed, &candidate, &plan).unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "payload_shape_inventory_mismatch"
        }
    );
}

/// Build a single-bone, single-instance `Document` with no `skin_ibms`
/// and no `Bone::inverse_bind`, so `instance_bind` must fall through to
/// `document.assets.source_skeleton` evidence — exactly the glTF
/// "skin declares no `inverseBindMatrices` accessor" shape, where the
/// format default is an identity inverse-bind matrix per joint.
fn absent_inverse_bind_document(
    status: SourceInverseBindAccessorStatus,
    coverage: SourceSkeletonCoverage,
) -> Document {
    Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "bone0".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: Vec::new(),
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
                source_node_index: 0,
                node: 0,
                mesh: 0,
                skin_joints: vec![0],
                skin_ibms: Vec::new(),
            }],
            source_skeleton: SourceSkeletonAssets {
                coverage,
                nodes: vec![SourceNodeAsset {
                    source_node_index: 0,
                    name: None,
                    parent_source_node_index: None,
                    scene_root_indices: vec![0],
                    local_rest: SourceNodeLocalRest::Trs {
                        translation: Vec3::ZERO,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::ONE,
                    },
                    bone: Some(0),
                }],
                skins: vec![SourceSkinAsset {
                    source_skin_index: 0,
                    name: None,
                    skeleton_root_source_node_index: None,
                    joint_source_node_indices: vec![0],
                    inverse_bind_accessor: SourceInverseBindAccessor {
                        status,
                        declared_count: None,
                        matrices: Vec::new(),
                    },
                    attachments: vec![SourceSkinAttachment {
                        source_node_index: 0,
                        source_mesh_index: Some(0),
                    }],
                }],
            },
            ..SceneAssets::default()
        },
        source: Default::default(),
    }
}

#[test]
fn absent_inverse_bind_accessor_with_complete_coverage_resolves_to_identity() {
    let doc = absent_inverse_bind_document(
        SourceInverseBindAccessorStatus::Absent,
        SourceSkeletonCoverage::Complete,
    );
    let instance = &doc.assets.instances[0];
    assert_eq!(instance_bind(&doc, instance, 0, 0), Ok(Mat4::IDENTITY));
}

#[test]
fn malformed_inverse_bind_accessor_status_still_rejects_rather_than_defaulting() {
    for status in [
        SourceInverseBindAccessorStatus::EmptyAccessor,
        SourceInverseBindAccessorStatus::CountMismatch,
        SourceInverseBindAccessorStatus::Unreadable,
    ] {
        let doc = absent_inverse_bind_document(status, SourceSkeletonCoverage::Complete);
        let instance = &doc.assets.instances[0];
        assert!(matches!(
            instance_bind(&doc, instance, 0, 0),
            Err(ScaleError::MissingInverseBind { node: 0 })
        ));
    }
}

#[test]
fn absent_inverse_bind_accessor_with_incomplete_coverage_still_rejects() {
    let doc = absent_inverse_bind_document(
        SourceInverseBindAccessorStatus::Absent,
        SourceSkeletonCoverage::Unavailable,
    );
    let instance = &doc.assets.instances[0];
    assert!(matches!(
        instance_bind(&doc, instance, 0, 0),
        Err(ScaleError::MissingInverseBind { node: 0 })
    ));
}

// --- Revalidation at build/prove boundaries (hardening gap 5) -------

#[test]
fn build_scale_candidate_rejects_a_duplicate_clip_track_added_after_planning() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let mut mutated = doc.clone();
    let track = Track {
        bone: 1,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0],
        values: TrackValues::Vec3s(vec![Vec3::ZERO]),
    };
    mutated.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![track.clone(), track],
    });
    assert!(matches!(
        build_scale_candidate(&mutated, &plan).unwrap_err(),
        ScaleError::InvalidDocumentShape(DocumentShapeError::DuplicateClipTrack { .. })
    ));
}

#[test]
fn prove_scale_rejects_a_malformed_source_document_replayed_against_a_valid_candidate() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut malformed_source = doc.clone();
    malformed_source.assets.instances[0].mesh = 99;
    assert!(matches!(
        prove_scale(&malformed_source, &candidate, &plan).unwrap_err(),
        ScaleError::InvalidDocumentShape(DocumentShapeError::MeshInstanceShape {
            violation: MeshInstanceShapeViolation::MeshIndexOutOfRange,
            ..
        })
    ));
}

// --- Candidate proof structure parity (hardening gap 6) -------------

#[test]
fn prove_scale_rejects_a_candidate_missing_a_source_clip() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO]),
        }],
    });
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut dropped = candidate.document().clone();
    dropped.clips.clear();
    let dropped = ScaleCandidate { document: dropped };
    assert!(matches!(
        prove_scale(&doc, &dropped, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "clip_count_mismatch"
        }
    ));
}

#[test]
fn prove_scale_rejects_a_candidate_with_an_extra_track_not_present_in_source() {
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut extended = candidate.document().clone();
    extended.clips.push(Clip {
        name: "extra".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO]),
        }],
    });
    let extended = ScaleCandidate { document: extended };
    assert!(matches!(
        prove_scale(&doc, &extended, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "clip_count_mismatch"
        }
    ));
}

#[test]
fn prove_scale_rejects_a_candidate_whose_track_times_differ_from_the_source() {
    // `prove_scale` does not require its two documents to be the same
    // one: a caller can build a candidate from one document and prove
    // it against another. Track times are the sampling grid *both*
    // sides are read on, so a candidate that agrees on track identity,
    // count, property and interpolation but disagrees on `times` would
    // have every sampled obligation -- key, cubic interior, trajectory,
    // skin and bounds -- comparing values drawn from different
    // instants. That proves nothing, so it is a structure mismatch
    // rather than a residual.
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 3.0, 0.0)]),
        }],
    });
    let capability = complete_capability();
    // Whole-document conversion by `0.01`.
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // The candidate is otherwise *correct*: both affected translation
    // keys carry exactly `0.01x` the authored value, and it proves.
    let TrackValues::Vec3s(built) = &candidate.document().clips[0].tracks[0].values else {
        panic!("translation track must hold Vec3 values");
    };
    assert!((built[0] - Vec3::new(0.0, 0.01, 0.0)).length() < 1e-9);
    assert!((built[1] - Vec3::new(0.0, 0.03, 0.0)).length() < 1e-9);
    prove_scale(&doc, &candidate, &plan).unwrap();

    // Move the second key from `1.0s` to `2.0s` and change nothing
    // else. Track count, `(bone, property, interpolation)` and the
    // value count all still match the source, so neither a count
    // mismatch nor a value residual can fire ahead of the time check:
    // the disagreeing sampling grid is the only thing left to catch.
    let mut retimed = candidate.document().clone();
    retimed.clips[0].tracks[0].times = vec![0.0, 2.0];
    assert_eq!(
        retimed.clips[0].tracks[0].values.len(),
        doc.clips[0].tracks[0].values.len(),
        "only the sampling grid may differ"
    );
    let retimed = ScaleCandidate { document: retimed };
    assert_eq!(
        prove_scale(&doc, &retimed, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "track_shape_mismatch"
        }
    );
}

// --- Authored (unnormalized) rotation equality ----------------------

#[test]
fn an_authored_rest_rotation_with_magnitude_below_one_is_not_a_rotation_residual() {
    // A routine authored glTF quaternion (a 45-degree turn about Y) whose
    // stored magnitude is `1 - 4e-8`, which `invariant-9` forbids the
    // loader from renormalizing. Comparing it to an untouched copy of
    // itself as an *angle* reports `6.9e-4`, `69x` the `1e-5` tolerance,
    // and rejects a correct candidate. As the equality test it actually
    // is, the residual is exactly zero.
    let unnormalized = Quat::from_xyzw(0.0, 0.3826834, 0.0, 0.9238795);
    assert!(
        unnormalized.as_dquat().length() < 1.0,
        "fixture quaternion must be shorter than unit length"
    );
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        RigNode {
            parent: Some(0),
            source_node_index: 1,
            translation: Vec3::new(0.0, 1.0, 0.0),
            rotation: unnormalized,
            scale: Vec3::ONE,
        },
    ];
    let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.rest_rotation.max(), 0.0);
}

#[test]
fn a_genuinely_rewritten_rest_rotation_still_fails_proof() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    broken.skeleton.bones[1].rest.rotation = Quat::from_rotation_y(0.5);
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::RestRotation,
            ..
        }
    ));
}

#[test]
fn a_rest_rotation_error_is_bounded_by_the_declared_angle_not_twice_it() {
    // DESIGN.md Appendix D §D.1 declares "shortest-path rotation residual
    // is at most `1e-5` radians". The residual is *measured* as a
    // double-cover-aware quaternion chord `|q1 - q2| = 2 * sin(theta / 4)`
    // — which is `theta / 2` to first order — so checking that chord
    // against the declared *angle* accepted a genuine `2e-5 rad` rotation
    // error: fail-open by exactly two.
    //
    // Both probes are literal. A rotation of `theta` about Y is
    // `(0, sin(theta / 2), 0, cos(theta / 2))`; at these angles
    // `cos(theta / 2)` is within `6e-11` of one and rounds to exactly
    // `1.0f32`, so the authored value is `(0, theta / 2, 0, 1)` and the
    // angle it represents is `2 * atan2(theta / 2, 1) = theta` to far
    // better than the `1e-9` bands asserted below. The source rotation is
    // the identity, so each doctored quaternion's own angle *is* the
    // residual under test.
    let doc = rig_document(&rest_only_leaf_rig(), &[1], 0, Mat4::IDENTITY);
    assert_eq!(doc.skeleton.bones[2].rest.rotation, Quat::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert_eq!(plan.tolerance_policy().rotation_residual_radians, 1e-5);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // Bone 2 is a leaf carrying no skin slot, no mesh vertex and no
    // track, so rotating it moves no descendant origin, no skin palette
    // and no sampled pose: the rest-rotation obligation is the only one
    // that can see either probe.
    let rotate_leaf = |half_angle: f32| {
        let mut broken = candidate.document().clone();
        broken.skeleton.bones[2].rest.rotation = Quat::from_xyzw(0.0, half_angle, 0.0, 1.0);
        ScaleCandidate { document: broken }
    };

    // `9e-6 rad`: inside the declared bound, and reported *as* `9e-6`
    // rather than as the `4.5e-6` chord it was measured from — the
    // reported field carries the unit its name and the §D.6 evidence
    // contract promise.
    let inside = rotate_leaf(4.5e-6);
    let proof = prove_scale(&doc, &inside, &plan).unwrap();
    assert!(
        (proof.rest_rotation.max() - 9.0e-6).abs() < 1e-9,
        "residual {} is not the 9e-6 radian angle it measures",
        proof.rest_rotation.max()
    );

    // `1.1e-5 rad`: outside the declared bound, but only a `5.5e-6`
    // chord — the value a chord-against-radians comparison accepted.
    let outside = rotate_leaf(5.5e-6);
    let error = prove_scale(&doc, &outside, &plan).unwrap_err();
    let ScaleError::ProofResidualExceeded {
        kind,
        observed,
        tolerance,
    } = error
    else {
        panic!("expected a residual rejection, got {error:?}");
    };
    assert_eq!(kind, ProofResidualKind::RestRotation);
    assert_eq!(tolerance, 1e-5);
    assert!(
        (observed - 1.1e-5).abs() < 1e-9,
        "observed {observed} is not the 1.1e-5 radian angle it measures"
    );
}

#[test]
fn a_rest_rotation_chord_above_two_saturates_at_two_pi_instead_of_reporting_nan() {
    // `quat_residual_radians` inverts the chord relation
    // `chord = 2 * sin(theta / 4)` as `theta = 4 * asin(chord / 2)`, whose
    // domain runs out at `chord = 2`. No pair of *unit* quaternions can
    // reach that (the double-cover minimum is at most `sqrt(2)`), but an
    // authored non-unit value can — and `invariant-9` forbids the loader
    // from normalizing one away — so the conversion clamps and saturates
    // at `4 * asin(1) = 2 * pi` rather than handing `asin` an
    // out-of-domain argument and reporting `NaN`.
    //
    // The pair fails closed either way: `2 * pi` and `NaN` both exceed the
    // `1e-5` bound (`check_residual` rejects a non-finite observation
    // outright). What is pinned here is therefore the *reported* residual
    // — the value DESIGN.md Appendix D §D.6 requires evidence to publish
    // next to the tolerance policy — not the accept/reject outcome.
    //
    // Both operands are literal. The source leaf rotation is exactly the
    // identity `(0, 0, 0, 1)` and the candidate's is the non-unit
    // `(0, 0, 0, -4)`, so the double-cover-aware chord is
    // `min(|(0, 0, 0, 5)|, |(0, 0, 0, -3)|) = 3`: `chord / 2 = 1.5` is
    // genuinely outside `asin`'s domain, and the clamp is the only thing
    // between this pair and a `NaN`.
    let doc = rig_document(&rest_only_leaf_rig(), &[1], 0, Mat4::IDENTITY);
    assert_eq!(doc.skeleton.bones[2].rest.rotation, Quat::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert_eq!(plan.tolerance_policy().rotation_residual_radians, 1e-5);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // Bone 2 is a leaf carrying no skin slot, no mesh vertex and no track,
    // so the rest-rotation obligation is the only one that can observe it.
    // A quaternion of the form `(0, 0, 0, w)` also leaves the world matrix
    // derived from it at the identity, so not even the rest-*translation*
    // residual of this node moves: the saturated value below is reported
    // by the rotation obligation and nothing else.
    let mut broken = candidate.document().clone();
    broken.skeleton.bones[2].rest.rotation = Quat::from_xyzw(0.0, 0.0, 0.0, -4.0);
    let broken = ScaleCandidate { document: broken };

    let error = prove_scale(&doc, &broken, &plan).unwrap_err();
    let ScaleError::ProofResidualExceeded {
        kind,
        observed,
        tolerance,
    } = error
    else {
        panic!("expected a residual rejection, got {error:?}");
    };
    assert_eq!(kind, ProofResidualKind::RestRotation);
    assert_eq!(tolerance, 1e-5);
    assert!(
        observed.is_finite(),
        "saturated residual {observed} must never be NaN"
    );
    // Exact, not approximate: `4.0 * asin(1.0)` is `4 * FRAC_PI_2`, and
    // scaling by a power of two is exact, so the saturation value is
    // bit-for-bit `TAU`.
    assert_eq!(observed, std::f64::consts::TAU);
}

#[test]
fn the_reported_rest_rotation_residual_is_the_maximum_not_the_last_node_seen() {
    // `ScaleProof::rest_rotation.max()` is a *maximum* over the
    // affected nodes — so a proof that reported the last affected node's
    // residual instead of the largest would publish a smaller number than
    // it observed. #284 will freeze this as a published §D.6 evidence
    // field, which is exactly what makes
    // accept/reject-unchanged-but-record-false a defect. Like
    // `unit_scale.max()`, this one is checked against a fixed policy
    // bound rather than a before/after-derived tolerance, so it reaches
    // the shared fold through `record_and_check` rather than
    // `check_and_track`.
    //
    // An earlier revision left this unpinned on the grounds that no build
    // path writes `rest.rotation`, so the residual is always zero. That
    // reasoning is about the *builder*; the obligation exists to catch
    // candidates the builder did not produce, and this file hand-builds
    // `ScaleCandidate { document }` in several places, including the two
    // rotation tests above. The build path has nothing to do with it.
    //
    // The rig puts the only nonzero residual on the *first* of two inert
    // leaves, so the last node folded reports exactly zero:
    //
    //   bone 0  root, no rotation error                  residual 0
    //   bone 1  skinned joint, no rotation error          residual 0
    //   bone 2  inert leaf, rotated                       residual theta
    //   bone 3  inert leaf, left identity                 residual 0
    //
    // A rotation of `theta` about X is `(sin(theta/2), 0, 0,
    // cos(theta/2))`; at `theta = 2^-17` the cosine is within `2^-36` of
    // one and rounds to exactly `1.0f32`, so the authored quaternion is
    // the literal `(2^-18, 0, 0, 1)`. Against an identity source rotation
    // the double-cover-aware chord is `|(2^-18, 0, 0, 0)| = 2^-18`
    // exactly, and the reported angle is
    //
    //   4 * asin(2^-18 / 2) = 4 * asin(2^-19) = 2^-17 = 7.62939453125e-6
    //
    // to within `2^-58` (the cubic term of `asin`), which is far inside
    // the `1e-15` band asserted below and comfortably under the `1e-5`
    // policy bound, so the candidate proves rather than being rejected.
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        rig(Some(0), 2, Vec3::new(3.0, 0.0, 0.0)),
        rig(Some(0), 3, Vec3::new(0.0, 0.0, 3.0)),
    ];
    let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    // The order the maximum is folded in: bone 3 is last.
    assert_eq!(plan.affected_nodes(), &[0, 1, 2, 3]);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // Bones 2 and 3 are leaves carrying no skin slot, no mesh vertex and
    // no track, so no descendant origin, skin palette or sampled pose
    // moves and the rest-rotation obligation is the only one that can see
    // either of them.
    let mut broken = candidate.document().clone();
    broken.skeleton.bones[2].rest.rotation = Quat::from_xyzw(2f32.powi(-18), 0.0, 0.0, 1.0);
    assert_eq!(broken.skeleton.bones[3].rest.rotation, Quat::IDENTITY);
    let broken = ScaleCandidate { document: broken };

    let proof = prove_scale(&doc, &broken, &plan).unwrap();
    assert!(
        (proof.rest_rotation.max() - 2f64.powi(-17)).abs() < 1e-15,
        "residual {} is not the 2^-17 radian angle bone 2 carries",
        proof.rest_rotation.max()
    );
    // Stated as its own inequality so the assertion above cannot be read
    // as "whatever the last node happened to produce": bone 3's residual,
    // the one a last-node fold would publish, is exactly zero.
    assert!(proof.rest_rotation.max() > 0.0);
}

// --- f32 representability of the declared factor --------------------

#[test]
fn a_factor_that_annihilates_or_overflows_at_the_f32_boundary_rejects_at_plan_time() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    // `1e-50` narrows to `0.0f32`: building would multiply every
    // translation, mesh `POSITION` and inverse-bind translation by zero
    // and every proof residual would still be exactly zero, because
    // `0 == 0 * 0`. `1e40` narrows to `inf`.
    for factor in [1e-50, 1e40] {
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor },
            document: &doc,
            capability: &capability,
        };
        assert!(
            matches!(
                plan_scale(&request).unwrap_err(),
                ScaleError::FactorNotRepresentable { .. }
            ),
            "factor {factor} was not rejected"
        );
    }
}

#[test]
fn a_rest_bind_factor_whose_reciprocal_overflows_f32_rejects_at_plan_time() {
    // `1e-40` narrows to a nonzero `f32` subnormal, so the declared
    // factor itself passes; the basis correction `C = scale(1 / s)` the
    // proof derives from it does not.
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1e-40,
        },
        document: &doc,
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&request).unwrap_err(),
        ScaleError::FactorNotRepresentable {
            declared: 1e-40,
            ..
        }
    ));
}

#[test]
fn a_non_finite_residual_fails_closed_instead_of_comparing_false() {
    // `NaN > tolerance` is `false`, so an unguarded comparison reports a
    // `NaN` residual as a pass.
    //
    // `-inf` is in the list because it is the *only* case that separates
    // the `!observed.is_finite()` the guard is written with from the
    // narrower `observed.is_nan()`: `+inf > tolerance` is true, so the
    // comparison already rejects a positive infinity with or without a
    // guard, and a `NaN`-only guard would still pass every other case
    // here. See `check_residual`'s own note for why the wider spelling is
    // kept even though no caller in this module can hand it a negative
    // residual.
    for (observed, tolerance) in [
        (f64::NAN, 1.0),
        (f64::INFINITY, 1.0),
        (f64::NEG_INFINITY, 1.0),
        (0.0, f64::NAN),
        (0.0, f64::INFINITY),
    ] {
        assert!(
            check_residual(ProofResidualKind::Bounds, observed, tolerance).is_err(),
            "observed {observed} tolerance {tolerance} passed"
        );
    }
}

// --- f64 affine classification --------------------------------------

#[test]
fn a_shear_only_f64_can_see_is_still_classified_as_sheared() {
    // Column pair whose `f32` dot product is `-9.98e-6` (inside the
    // `1e-5` threshold) but whose `f64` dot product is `-1.00043e-5`
    // (outside it). Evaluating the classifier's dot products in `f32`
    // and casting afterwards accepts this basis as orthogonal.
    let c0 = Vec3::new(0.12792248, -0.99066633, -0.047073245);
    let c1 = Vec3::new(-0.34637994, -0.00016034879, -0.93809813);
    let c2 = Vec3::new(0.92933476, 0.13630849, -0.3431568);
    assert!((c1.dot(c2) as f64).abs() < 1e-5, "f32 dot is inside band");
    assert!(
        (c1.as_dvec3().dot(c2.as_dvec3())).abs() > 1e-5,
        "f64 dot is outside band"
    );
    let error = reject_case(|rest| {
        *rest = SourceNodeLocalRest::Matrix(Mat4::from_cols(
            c0.extend(0.0),
            c1.extend(0.0),
            c2.extend(0.0),
            Vec4::W,
        ));
    });
    assert!(
        matches!(
            error,
            ScaleError::InvalidAffineDomain {
                reason: AffineDomainViolation::Sheared,
                ..
            }
        ),
        "unexpected error {error:?}"
    );
}

/// The column pair a single shear term is aimed at.
#[derive(Clone, Copy, Debug)]
enum ShearPair {
    /// `dot01` — `columns[0] · columns[1]`.
    XY,
    /// `dot02` — `columns[0] · columns[2]`.
    XZ,
    /// `dot12` — `columns[1] · columns[2]`.
    YZ,
}

/// A basis carrying exactly one shear term `s`, in the named column pair.
///
/// Two columns are the unit axes they name; the third adds `s` in one
/// foreign axis:
///
/// ```text
///   XY: (1, 0, 0) (s, 1, 0) (0, 0, 1)   dot01 = s, dot02 = 0, dot12 = 0
///   XZ: (1, 0, 0) (0, 1, 0) (s, 0, 1)   dot02 = s, dot01 = 0, dot12 = 0
///   YZ: (1, 0, 0) (0, 1, 0) (0, s, 1)   dot12 = s, dot01 = 0, dot02 = 0
/// ```
///
/// Each zero above is structural — every term of those two dot products
/// has a literal `0.0` factor — so the pairs a fixture is *not* aiming at
/// stay exactly zero for every `s`, and only the named comparison can
/// decide.
fn single_shear_basis(pair: ShearPair, s: f32) -> Mat3 {
    match pair {
        ShearPair::XY => Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(s, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ),
        ShearPair::XZ => Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(s, 0.0, 1.0),
        ),
        ShearPair::YZ => Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, s, 1.0),
        ),
    }
}

/// Drive [`single_shear_basis`] through `classify_affine` at both signs of
/// both magnitudes, asserting `Sheared` outside the band and acceptance
/// inside it.
///
/// The shape is chosen so that *no other check in `classify_affine` can
/// fire*, at either magnitude and either sign. With `s = ±2^-15`:
///
/// * **NonFinite** — every entry is a finite literal.
/// * **Singular** — the sheared column is `unit_axis + s * other_axis`,
///   and `other_axis` is one of the two columns left untouched, so the
///   triple product is unchanged from the identity's: `determinant` is
///   exactly `1.0` for all three pairs. `axis_product` is
///   `sqrt(1 + 2^-30) < 1.000000001`, so the singular threshold is
///   `1e-6 * that`, six orders below `1.0`.
/// * **NonUniformScale** — the length multiset is
///   `(1, 1, sqrt(1 + 2^-30))` for all three pairs, i.e.
///   `(1, 1, 1.000000000465661…)`. The average is `1.000000000155220…`
///   and the largest deviation is `2/3 * 2^-31 = 3.10e-10`, against a
///   band of `1e-5 * max(average, length) > 1e-5` — four and a half
///   orders of headroom.
/// * **Reflected** — the determinant is `+1.0`, so even deleting the
///   orthogonality check outright yields `Ok`, never `Reflected`.
///
/// That leaves the orthogonality comparison as the only possible source
/// of a `Sheared` verdict. It fires because
/// `|s| = 2^-15 = 3.0517578125e-5` and the tolerance is
/// `1e-5 * average^2 = 1.0000000003…e-5` — the shear is 3.05x the bound.
/// At `s = ±2^-17 = ±7.62939453125e-6` the same arithmetic gives
/// `0.76x` the bound (lengths `(1, 1, sqrt(1 + 2^-34))`, tolerance
/// `1.0000000000…e-5`), so the in-band cases must be accepted: the
/// rejections above are the shear magnitude talking and not the shape.
fn assert_only_this_column_pair_decides_shear(pair: ShearPair) {
    let tol = ScaleTolerancePolicy::APPENDIX_D_V6;
    // Written as exact dyadics rather than decimal literals, so the
    // binary32 bits are readable straight off the page: `2^-15` and
    // `2^-17`.
    let out_of_band_magnitude = 2.0_f32.powi(-15);
    let in_band_magnitude = 2.0_f32.powi(-17);
    for sign in [1.0_f32, -1.0] {
        let out_of_band = sign * out_of_band_magnitude;
        assert_eq!(
            classify_affine(single_shear_basis(pair, out_of_band), &tol),
            Err(AffineDomainViolation::Sheared),
            "{pair:?} shear {out_of_band} was not rejected as sheared"
        );
        let in_band = sign * in_band_magnitude;
        assert!(
            classify_affine(single_shear_basis(pair, in_band), &tol).is_ok(),
            "{pair:?} shear {in_band} was rejected, so the shape and not \
                 the magnitude is what the out-of-band case rejects on"
        );
    }
}

#[test]
fn shear_isolated_to_the_x_y_column_pair_is_sheared_at_either_sign() {
    // Pins `dot01`'s own clause *and* its `.abs()`: with the absolute
    // value dropped, `dot01 = -2^-15` is not `> tolerance` and the other
    // two dot products are structurally zero, so the basis is accepted.
    assert_only_this_column_pair_decides_shear(ShearPair::XY);
}

#[test]
fn shear_isolated_to_the_x_z_column_pair_is_sheared_at_either_sign() {
    // Pins `dot02`'s clause and its `.abs()`. No other fixture isolates
    // `x·z`: the two column pairs either side of it stay exactly zero
    // here, so deleting this clause accepts the basis outright.
    assert_only_this_column_pair_decides_shear(ShearPair::XZ);
}

#[test]
fn shear_isolated_to_the_y_z_column_pair_is_sheared_at_either_sign() {
    // Pins `dot12`'s clause and its `.abs()`.
    // `a_shear_only_f64_can_see_is_still_classified_as_sheared` already
    // reaches this comparison, but through a dense basis in which all
    // three dot products are nonzero; this one isolates it, so the three
    // pairs are covered on equal terms.
    assert_only_this_column_pair_decides_shear(ShearPair::YZ);
}

// --- Per-value proof of every domain --------------------------------

/// `unit_rig` plus a rotation track and a three-vertex primitive whose
/// middle vertex is strictly interior to the skinned bounding box — the
/// two payloads no sampled obligation looks at.
fn payload_document() -> Document {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.assets.meshes[0].primitives[0] = Primitive {
        positions: vec![
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::ZERO,
            Vec3::new(-1.0, -1.0, -1.0),
        ],
        joints: vec![[0, 0, 0, 0]; 3],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
        ..Primitive::default()
    };
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![
                Quat::from_rotation_y(0.1),
                Quat::from_rotation_y(0.1),
            ]),
        }],
    });
    doc
}

fn whole_document_plan(document: &Document, capability: &ScaleCapabilityFacts) -> ScalePlan {
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document,
        capability,
    })
    .unwrap()
}

#[test]
fn a_rotation_key_rewritten_in_the_candidate_fails_proof() {
    // Reachable through the public API: `build_scale_candidate` and
    // `prove_scale` each take their document separately, so a candidate
    // can be built from a doctored copy and proved against the real
    // source. Rotation values are a domain both operations declare
    // untouched; nothing sampled would notice.
    let doc = payload_document();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let mut doctored = doc.clone();
    doctored.clips[0].tracks[0].values =
        TrackValues::Quats(vec![Quat::from_rotation_y(2.5), Quat::from_rotation_y(2.5)]);
    let candidate = build_scale_candidate(&doctored, &plan).unwrap();
    assert!(matches!(
        prove_scale(&doc, &candidate, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::TrackValue,
            ..
        }
    ));
}

#[test]
fn an_interior_mesh_vertex_moved_in_the_candidate_fails_proof() {
    let doc = payload_document();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let mut doctored = doc.clone();
    // Strictly inside the skinned bounding box, so the bounds obligation
    // is blind to it.
    doctored.assets.meshes[0].primitives[0].positions[1] = Vec3::new(0.5, 0.5, 0.5);
    let candidate = build_scale_candidate(&doctored, &plan).unwrap();
    assert!(matches!(
        prove_scale(&doc, &candidate, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::MeshPosition,
            ..
        }
    ));
}

#[test]
fn an_unsampled_translation_tangent_is_named_by_the_track_value_obligation() {
    // The rotation and mesh-position arms of `check_candidate_values`
    // each already name themselves (see the two tests above); its
    // `Vec3s` arm — every translation value and cubic tangent element —
    // did not, so the kind it reports was free to be any variant.
    //
    // glTF cubic evaluation of the segment `[k0, k1]` reads only the
    // *out*-tangent of `k0` and the *in*-tangent of `k1`. For a two-key
    // track that leaves `values[0]`, the in-tangent at the first key,
    // unread at every key time and every cubic interior time — and so
    // unread by the trajectory, skin and bounds obligations derived from
    // those samples. The direct per-element check is the only obligation
    // that can see this element at all, which is what makes the kind it
    // reports this test's subject rather than an artefact of ordering.
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![
                Vec3::new(0.0, 500.0, 0.0), // in-tangent @0 — never sampled
                Vec3::new(0.0, 1.0, 0.0),   // value @0
                Vec3::ZERO,                 // out-tangent @0 (`m0`)
                Vec3::ZERO,                 // in-tangent @1 (`m1`)
                Vec3::new(0.0, 1.0, 0.0),   // value @1
                Vec3::ZERO,                 // out-tangent @1
            ]),
        }],
    });
    let capability = complete_capability();
    // Whole-document conversion by `0.01`.
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    let TrackValues::Vec3s(values) = &mut broken.clips[0].tracks[0].values else {
        panic!("expected a vec3 track");
    };
    // A builder that left this one element un-rewritten: the candidate
    // keeps the source's `500` where `0.01 * 500 = 5` is expected.
    values[0] = Vec3::new(0.0, 500.0, 0.0);
    let broken = ScaleCandidate { document: broken };
    let error = prove_scale(&doc, &broken, &plan).unwrap_err();
    let ScaleError::ProofResidualExceeded {
        kind,
        observed,
        tolerance,
    } = error
    else {
        panic!("expected a proof residual, got {error:?}");
    };
    assert_eq!(kind, ProofResidualKind::TrackValue);
    // `|500 - 5| = 495`, against `1e-6 + 1e-5 * 500 = 5.001e-3`.
    assert!((observed - 495.0).abs() < 1e-9, "observed {observed}");
    assert!((tolerance - 5.001e-3).abs() < 1e-9, "tolerance {tolerance}");
}

#[test]
fn an_honest_candidate_proves_every_retained_payload_with_a_zero_residual() {
    let doc = payload_document();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.track_value.max(), 0.0);
    assert!(proof.mesh_position.max() < 1e-9);
}

#[test]
fn sample_times_are_harvested_from_every_animated_track_not_only_translation() {
    // `payload_document`'s only clip animates rotation. Harvesting sample
    // times from translation tracks alone leaves `sample_time_count` at
    // zero, making every sampled obligation vacuously true.
    let doc = payload_document();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.sample_time_count, 2);
}

// --- Base positions and bounds evidence -----------------------------

/// One unskinned mesh instance and no skinned instance at all: the
/// declared `base_mesh_positions` rewrite has no skinned bounds to be
/// proved through.
fn unskinned_document() -> Document {
    Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "bone0".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: Vec::new(),
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "mesh".into(),
                source_mesh_index: 0,
                primitives: vec![Primitive {
                    positions: vec![Vec3::new(1.0, 2.0, 3.0)],
                    ..Primitive::default()
                }],
            }],
            instances: vec![MeshInstance {
                source_node_index: 0,
                node: 0,
                mesh: 0,
                skin_joints: Vec::new(),
                skin_ibms: Vec::new(),
            }],
            ..SceneAssets::default()
        },
        source: Default::default(),
    }
}

#[test]
fn an_unskinned_document_does_not_declare_a_bounds_obligation_it_cannot_check() {
    let doc = unskinned_document();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert!(
        !plan
            .obligations()
            .contains(&ScaleProofObligation::SkinAndBounds)
    );
    // The combined obligation owns the shared instance walk: with no
    // skinned instance there is neither a `W_i * B_i` nor bounds evidence.
    assert!(plan.field_rows().iter().any(|row| matches!(
        (row.target, row.disposition),
        (
            ScaleFieldTarget::MeshPositions { .. },
            ScaleFieldDisposition::Rewrite(ScaleRewriteRule::WholeDocumentLength)
        )
    )));
}

#[test]
fn an_unskinned_documents_base_positions_are_proved_directly() {
    let doc = unskinned_document();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    assert!(
        (candidate.document().assets.meshes[0].primitives[0].positions[0]
            - Vec3::new(0.01, 0.02, 0.03))
        .length()
            < 1e-8
    );
    prove_scale(&doc, &candidate, &plan).unwrap();

    // A candidate that silently skipped the declared rewrite must fail,
    // even though there is no skinned instance and therefore no bounds.
    let mut unrewritten = candidate.document().clone();
    unrewritten.assets.meshes[0].primitives[0].positions[0] = Vec3::new(1.0, 2.0, 3.0);
    let unrewritten = ScaleCandidate {
        document: unrewritten,
    };
    assert!(matches!(
        prove_scale(&doc, &unrewritten, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::MeshPosition,
            ..
        }
    ));
}

#[test]
fn a_replayed_plan_cannot_lose_its_bounds_evidence() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::SkinAndBounds)
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    // Replay the plan against a document pair that no longer carries the
    // skinned instance its bounds obligation was declared from. Both
    // sides lose it, so source/candidate structure still agrees; the
    // source-derived obligation inventory is what must fail closed.
    let mut unskinned = doc.clone();
    unskinned.assets.instances[0].skin_joints.clear();
    unskinned.assets.instances[0].skin_ibms.clear();
    let mut unskinned_candidate = candidate.into_document();
    unskinned_candidate.assets.instances[0].skin_joints.clear();
    unskinned_candidate.assets.instances[0].skin_ibms.clear();
    let candidate = ScaleCandidate {
        document: unskinned_candidate,
    };
    assert_eq!(
        prove_scale(&unskinned, &candidate, &plan).unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "proof_obligations_mismatch"
        }
    );
}

// --- Evidence gating, per obligation (issue #302) ---------------------

/// `multi_joint_document` with its two affected translation tracks
/// replaced by one affected *rotation* track.
///
/// That clip still yields sample times — so the trajectory obligation has
/// evidence — while carrying no translation payload for the key
/// obligation to compare and no cubic segment to take an interior of.
/// It is the one document shape that separates `prove_trajectories` from
/// its two siblings, which is what makes each of the three refusals below
/// reachable on its own.
fn rotation_only_clip_document() -> Document {
    let mut doc = multi_joint_document();
    doc.clips[0].tracks = vec![Track {
        bone: 1,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Quats(vec![Quat::IDENTITY, Quat::IDENTITY]),
    }];
    doc
}

/// Strip every clip from a source and a candidate together, so the pair
/// still satisfies `validate_candidate_structure` and what fails is the
/// missing obligation evidence rather than clip-count parity.
fn without_clips(source: &Document, candidate: ScaleCandidate) -> (Document, ScaleCandidate) {
    let mut source = source.clone();
    source.clips.clear();
    let mut document = candidate.into_document();
    document.clips.clear();
    (source, ScaleCandidate { document })
}

/// Assert both public plan-replay boundaries reject `source` before a
/// stale inventory can select the builder or proof walks.
fn assert_replayed_inventory_mismatch(source: &Document, plan: &ScalePlan) {
    let expected = ScaleError::PlanDocumentMismatch {
        reason: "proof_obligations_mismatch",
    };
    assert_eq!(build_scale_candidate(source, plan).unwrap_err(), expected);
    assert_eq!(
        prove_scale(
            source,
            &ScaleCandidate {
                document: source.clone(),
            },
            plan,
        )
        .unwrap_err(),
        expected
    );
}

#[test]
fn the_clip_driven_obligations_are_declared_only_by_the_tracks_that_evidence_them() {
    let capability = complete_capability();

    // No clips at all: none of the three has anything to read.
    let unanimated = compensated_document();
    let unanimated_plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &unanimated,
        capability: &capability,
    })
    .unwrap();
    assert!(unanimated.clips.is_empty());
    let obligations = unanimated_plan.obligations().to_vec();
    assert!(!obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!obligations.contains(&ScaleProofObligation::CubicInteriors));
    assert!(!obligations.contains(&ScaleProofObligation::Trajectories));

    // An affected rotation track: a sample time exists, a translation
    // payload does not.
    let rotation_only = rotation_only_clip_document();
    let obligations = multi_joint_plan(&rotation_only, &capability)
        .obligations()
        .to_vec();
    assert!(!obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!obligations.contains(&ScaleProofObligation::CubicInteriors));
    assert!(obligations.contains(&ScaleProofObligation::Trajectories));

    // An affected *linear* translation track: keys and trajectories have
    // evidence; a linear segment has no interior time to bound.
    let mut linear_only = multi_joint_document();
    linear_only.clips[0].tracks.truncate(1);
    assert_eq!(
        linear_only.clips[0].tracks[0].interpolation,
        Interpolation::Linear
    );
    let obligations = multi_joint_plan(&linear_only, &capability)
        .obligations()
        .to_vec();
    assert!(obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!obligations.contains(&ScaleProofObligation::CubicInteriors));
    assert!(obligations.contains(&ScaleProofObligation::Trajectories));

    // And the unmodified fixture, which carries both track kinds, still
    // declares all three — so the three negatives above are the evidence
    // gate and not a planner that stopped declaring them.
    let animated = multi_joint_document();
    let obligations = multi_joint_plan(&animated, &capability)
        .obligations()
        .to_vec();
    assert!(obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(obligations.contains(&ScaleProofObligation::CubicInteriors));
    assert!(obligations.contains(&ScaleProofObligation::Trajectories));
}

#[test]
fn a_replayed_plan_cannot_lose_its_key_evidence() {
    let doc = multi_joint_document();
    let capability = complete_capability();
    let plan = multi_joint_plan(&doc, &capability);
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::KeyTranslations)
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    prove_scale(&doc, &candidate, &plan).unwrap();

    let (clipless, clipless_candidate) = without_clips(&doc, candidate);
    assert_eq!(
        prove_scale(&clipless, &clipless_candidate, &plan).unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "proof_obligations_mismatch"
        }
    );
}

#[test]
fn a_replayed_plan_cannot_lose_its_cubic_evidence() {
    let doc = multi_joint_document();
    let capability = complete_capability();
    let plan = multi_joint_plan(&doc, &capability);
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::CubicInteriors)
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // Drop only the cubic track, from both sides. The linear translation
    // track survives, so the key obligation still has its evidence and
    // the refusal below is the cubic one specifically — the two are
    // reported separately on purpose.
    let mut source = doc.clone();
    source.clips[0].tracks.truncate(1);
    let mut document = candidate.into_document();
    document.clips[0].tracks.truncate(1);
    assert_eq!(
        prove_scale(&source, &ScaleCandidate { document }, &plan).unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "proof_obligations_mismatch"
        }
    );
}

#[test]
fn a_replayed_plan_cannot_lose_its_trajectory_evidence() {
    // `prove_trajectories` is the widest of the three, so its refusal is
    // only reachable from a plan that declares it *without* the other
    // two — otherwise the key or cubic refusal fires first and this one
    // would never be observed.
    let doc = rotation_only_clip_document();
    let capability = complete_capability();
    let plan = multi_joint_plan(&doc, &capability);
    let obligations = plan.obligations().to_vec();
    assert!(obligations.contains(&ScaleProofObligation::Trajectories));
    assert!(!obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!obligations.contains(&ScaleProofObligation::CubicInteriors));
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    prove_scale(&doc, &candidate, &plan).unwrap();

    let (clipless, clipless_candidate) = without_clips(&doc, candidate);
    assert_eq!(
        prove_scale(&clipless, &clipless_candidate, &plan).unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "proof_obligations_mismatch"
        }
    );
}

#[test]
fn a_replayed_plan_cannot_gain_trajectory_evidence() {
    let doc = compensated_document();
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let before = plan.obligations().to_vec();
    assert!(!before.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!before.contains(&ScaleProofObligation::CubicInteriors));
    assert!(!before.contains(&ScaleProofObligation::Trajectories));

    let mut gained = doc.clone();
    gained.clips.push(Clip {
        name: "gained_rotation".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![Quat::IDENTITY, Quat::IDENTITY]),
        }],
    });
    let after = compensated_rest_bind_plan(&gained, &capability)
        .obligations()
        .to_vec();
    assert!(!after.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!after.contains(&ScaleProofObligation::CubicInteriors));
    assert!(after.contains(&ScaleProofObligation::Trajectories));
    assert_replayed_inventory_mismatch(&gained, &plan);
}

#[test]
fn a_replayed_plan_cannot_gain_key_evidence() {
    let doc = rotation_only_clip_document();
    let capability = complete_capability();
    let plan = multi_joint_plan(&doc, &capability);
    let before = plan.obligations().to_vec();
    assert!(!before.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!before.contains(&ScaleProofObligation::CubicInteriors));
    assert!(before.contains(&ScaleProofObligation::Trajectories));

    let mut gained = doc.clone();
    gained.clips[0].tracks.push(linear_translation_track());
    let after = multi_joint_plan(&gained, &capability)
        .obligations()
        .to_vec();
    assert!(after.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!after.contains(&ScaleProofObligation::CubicInteriors));
    assert!(after.contains(&ScaleProofObligation::Trajectories));
    assert_replayed_inventory_mismatch(&gained, &plan);
}

#[test]
fn a_replayed_plan_cannot_gain_cubic_evidence() {
    let mut doc = multi_joint_document();
    doc.clips[0].tracks.truncate(1);
    let capability = complete_capability();
    let plan = multi_joint_plan(&doc, &capability);
    let before = plan.obligations().to_vec();
    assert!(before.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!before.contains(&ScaleProofObligation::CubicInteriors));
    assert!(before.contains(&ScaleProofObligation::Trajectories));

    let mut gained = doc.clone();
    gained.clips[0].tracks.push(cubic_rotation_track());
    let after = multi_joint_plan(&gained, &capability)
        .obligations()
        .to_vec();
    assert!(after.contains(&ScaleProofObligation::KeyTranslations));
    assert!(after.contains(&ScaleProofObligation::CubicInteriors));
    assert!(after.contains(&ScaleProofObligation::Trajectories));
    assert_replayed_inventory_mismatch(&gained, &plan);
}

/// A two-key `CUBICSPLINE` *rotation* track on bone 2, with zero
/// tangents so the interpolation is well conditioned and the pose it
/// produces is not what is under test.
///
/// What matters is only that it is a cubic segment carrying no
/// translation payload: it is what produces an interior time, and it is
/// not what the comparison at that time reads.
fn cubic_rotation_track() -> Track {
    let zero = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
    Track {
        bone: 2,
        property: Property::Rotation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 1.0],
        values: TrackValues::Quats(vec![
            zero,                       // in-tangent @0
            Quat::IDENTITY,             // value @0
            zero,                       // out-tangent @0
            zero,                       // in-tangent @1
            Quat::from_rotation_z(0.5), // value @1
            zero,                       // out-tangent @1
        ]),
    }
}

/// The same linear translation track `multi_joint_document` carries on
/// bone 1, so the fixtures below differ from that document only where
/// they mean to.
fn linear_translation_track() -> Track {
    Track {
        bone: 1,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::new(0.0, 100.0, 0.0), Vec3::new(0.0, 200.0, 0.0)]),
    }
}

#[test]
fn the_cubic_obligations_two_halves_must_meet_inside_one_clip_and_neither_need_be_the_other() {
    // The cubic obligation is the only one whose evidence is a
    // *conjunction*: an interior time has to exist, and something the
    // comparison reads has to exist at it. Interior times are harvested
    // per clip (`clip_sample_times`) and compared against that clip's
    // tracks (`check_track_value_residual`), so the two halves must meet
    // inside one clip — and neither half has to be the other's track.
    //
    // `multi_joint_document` cannot show either fact: its cubic track *is*
    // a translation track, in the same clip as another translation track,
    // so "per clip", "document-wide", "cubic segment alone" and "the cubic
    // must be the translation track" all report `true` on it alike.
    let capability = complete_capability();

    // Split across two clips: clip 0 has the payload and no cubic
    // segment, clip 1 has the cubic segment and no payload. Neither clip
    // has both, so there is no cubic-interior evidence — while the
    // document as a whole carries one of each, which is exactly what a
    // document-wide conjunction would (wrongly) accept, and what dropping
    // the translation half of the conjunction would accept too.
    let mut split = multi_joint_document();
    split.clips[0].tracks = vec![linear_translation_track()];
    split.clips.push(Clip {
        name: "cubic_rotation".into(),
        duration_s: 1.0,
        tracks: vec![cubic_rotation_track()],
    });
    let split_plan = multi_joint_plan(&split, &capability);
    let obligations = split_plan.obligations().to_vec();
    assert!(obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!obligations.contains(&ScaleProofObligation::CubicInteriors));
    assert!(obligations.contains(&ScaleProofObligation::Trajectories));
    // And it is an otherwise ordinary document, so the negative above is
    // the conjunction and not a document proof would have refused anyway.
    // Under a document-wide conjunction this same call returns `Ok` with
    // `cubic_interior.max()` published as `0.0` from zero comparisons —
    // issue #302's falsehood exactly.
    let split_candidate = build_scale_candidate(&split, &split_plan).unwrap();
    let proof = prove_scale(&split, &split_candidate, &split_plan).unwrap();
    assert_eq!(proof.cubic_interior.max(), 0.0);

    // The same two tracks in one clip: now the obligation has both halves,
    // and the cubic track supplying the interior time is a *rotation*
    // track while the translation read at that time is a different track
    // on a different bone. Requiring the cubic segment to be the
    // translation track itself would refuse this legal document.
    let mut shared = multi_joint_document();
    shared.clips[0].tracks = vec![linear_translation_track(), cubic_rotation_track()];
    let shared_plan = multi_joint_plan(&shared, &capability);
    let obligations = shared_plan.obligations().to_vec();
    assert!(obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(obligations.contains(&ScaleProofObligation::CubicInteriors));
    assert!(obligations.contains(&ScaleProofObligation::Trajectories));
    let shared_candidate = build_scale_candidate(&shared, &shared_plan).unwrap();
    // The declared obligation is honoured: `0.5` is the rotation track's
    // interior, and the translation track is read there.
    prove_scale(&shared, &shared_candidate, &shared_plan).unwrap();
}

#[test]
fn a_single_key_cubic_track_is_not_a_cubic_segment() {
    // `times.len() >= 2` is what makes the track a *segment*:
    // `clip_sample_times` takes interiors from `times.windows(2)`, so one
    // key yields none. A one-key cubic alongside a translation track has
    // every other mark of cubic evidence — cubic interpolation, an
    // affected bone, a translation payload in the same clip — and still
    // produces nothing for the obligation to read.
    let mut doc = multi_joint_document();
    doc.clips[0].tracks = vec![
        linear_translation_track(),
        Track {
            bone: 2,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![
                Vec3::ZERO,                 // in-tangent @0
                Vec3::new(0.0, 100.0, 0.0), // value @0
                Vec3::ZERO,                 // out-tangent @0
            ]),
        },
    ];
    let capability = complete_capability();
    let plan = multi_joint_plan(&doc, &capability);
    let obligations = plan.obligations().to_vec();
    assert!(obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!obligations.contains(&ScaleProofObligation::CubicInteriors));
    assert!(obligations.contains(&ScaleProofObligation::Trajectories));

    // Declaring it on this document would publish a `0.0` cubic residual
    // from an interior-time loop with nothing to iterate.
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.cubic_interior.max(), 0.0);
}

/// A four-bone chain whose §D.2 closure is a strict *interior* subset,
/// in both hierarchies at once.
///
/// `bone0 -> bone1 -> bone2 -> bone3`, source-node numbering equal to
/// bone numbering, the compensating `0.01` on bone 2, and the skin's only
/// joint on bone 3. Rooting the rest/bind operation at source node 2
/// closes over `{2, 3}` — the root, its joint, and every descendant of
/// both — so bones 0 and 1 are outside the closure without either
/// hierarchy having to contradict the other.
fn mid_chain_closure_document() -> Document {
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        RigNode {
            parent: Some(1),
            source_node_index: 2,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(2), 3, Vec3::new(0.0, 100.0, 0.0)),
    ];
    let joint_world = Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0))
        * Mat4::from_scale(Vec3::splat(0.01))
        * Mat4::from_translation(Vec3::new(0.0, 100.0, 0.0));
    rig_document(&nodes, &[3], 0, joint_world.inverse())
}

/// [`plan_scale`] for [`mid_chain_closure_document`], rooted at the
/// interior source node the closure starts from.
fn mid_chain_closure_plan(document: &Document) -> ScalePlan {
    let capability = complete_capability();
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 2,
            expected_factor: 0.01,
        },
        document,
        capability: &capability,
    })
    .expect("the mid-chain rig plans")
}

#[test]
fn a_track_on_an_unaffected_bone_is_evidence_for_nothing() {
    // Every other fixture here animates a bone that is *in* the closure,
    // so the `affected` filter in `sampled_evidence` never decides
    // anything. `mid_chain_closure_document` is the exception: its closure
    // is `{2, 3}` of four bones, and bones 0 and 1 — genuinely outside it
    // in the projection *and* in the skeleton — are free to animate.
    let mut doc = mid_chain_closure_document();
    doc.clips.push(Clip {
        name: "unaffected".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![
                Vec3::ZERO,
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::ZERO,
            ]),
        }],
    });
    let plan = mid_chain_closure_plan(&doc);
    assert_eq!(plan.affected_nodes(), &[2, 3]);
    assert_eq!(doc.skeleton.bones.len(), 4);
    // A translation track, cubic, with two keys — every property the
    // three flags read — on a bone the closure does not contain.
    let obligations = plan.obligations().to_vec();
    assert!(!obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!obligations.contains(&ScaleProofObligation::CubicInteriors));
    assert!(!obligations.contains(&ScaleProofObligation::Trajectories));

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    prove_scale(&doc, &candidate, &plan).unwrap();
}

#[test]
fn the_replayed_plan_inventory_reads_key_evidence_not_a_neighbouring_field() {
    // The three sampled inventory fields differ only in which
    // `SampledEvidence` member each reads. Removing every clip takes all
    // three false together, so a key check accidentally reading
    // `sample_times` instead of `key_translations` would look correct.
    //
    // A rotation-only clip separates them: `sample_times` is true and
    // `key_translations` is false. The plan is built from a document whose
    // only track is a *linear* translation, so it declares `prove_keys`
    // without `prove_cubic_interiors` and the key gate is the first one
    // reached.
    let mut linear_only = multi_joint_document();
    linear_only.clips[0].tracks = vec![linear_translation_track()];
    let capability = complete_capability();
    let plan = multi_joint_plan(&linear_only, &capability);
    let obligations = plan.obligations().to_vec();
    assert!(obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(!obligations.contains(&ScaleProofObligation::CubicInteriors));
    assert!(obligations.contains(&ScaleProofObligation::Trajectories));

    // The replay source keeps a clip — so it still has sample times — and
    // carries no translation payload for the key obligation to read.
    let rotation_only = rotation_only_clip_document();
    assert_eq!(
        build_scale_candidate(&rotation_only, &plan).unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "proof_obligations_mismatch"
        }
    );
}

#[test]
fn an_unskinned_rest_bind_document_declares_neither_skin_nor_bounds() {
    // The combined obligation is evidence-gated on the rest/bind planner
    // too, and nothing here was covering that: every rest/bind skin/bounds
    // test uses a document that *does* carry a skinned instance, and the
    // `has_skinned_evidence` negatives all run on the whole-document
    // planner (`an_unskinned_document_does_not_declare_a_bounds
    // _obligation_it_cannot_check`).
    //
    // A rest/bind rebase does not need a mesh instance: the operation is
    // resolved from `source_skeleton.skins`, so a document that declares a
    // skin but instantiates no mesh through it is legal and must prove.
    // Declaring the obligation unconditionally here turns it into a refusal.
    let mut doc = multi_joint_document();
    doc.assets.instances.clear();
    let capability = complete_capability();
    let plan = multi_joint_plan(&doc, &capability);
    let obligations = plan.obligations().to_vec();
    assert!(!obligations.contains(&ScaleProofObligation::SkinAndBounds));
    // The clip-driven obligations are unaffected, so this is the skinned
    // evidence gate and not a planner that stopped declaring anything.
    assert!(obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(obligations.contains(&ScaleProofObligation::Trajectories));

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    // The document is sampled, so a declared skin/bounds obligation
    // here would have been reached rather than skipped for want of a
    // sample time: what the obligation turns on is the instance walk alone.
    assert!(proof.sample_time_count > 0);

    // The decisive stale-plan direction is evidence gained, not lost.
    // Re-add a skinned instance on the same already-affected joints: the
    // affected closure and attachment list stay identical, but the stale
    // plan still lacks the combined obligation. Before full inventory
    // parity, a candidate with a corrupted affected bind or vertex weight
    // skipped both skin and bounds and proved successfully.
    let skinned = multi_joint_document();
    let skinned_domain = derive_rest_bind_plan_domain(&skinned, 0, 0).unwrap();
    assert_eq!(skinned_domain.affected_nodes(), plan.affected_nodes());
    let expected = ScaleError::PlanDocumentMismatch {
        reason: "proof_obligations_mismatch",
    };
    assert_eq!(
        build_scale_candidate(&skinned, &plan).unwrap_err(),
        expected
    );

    let mut corrupted = build_rest_bind(&skinned, &plan).unwrap();
    corrupted.assets.instances[0].skin_ibms[0] = Mat4::IDENTITY;
    assert_eq!(
        prove_scale(
            &skinned,
            &ScaleCandidate {
                document: corrupted
            },
            &plan
        )
        .unwrap_err(),
        expected
    );
}

#[test]
fn a_closure_with_no_transform_only_attachment_does_not_declare_the_affine_obligation() {
    let capability = complete_capability();

    // Every affected node of `multi_joint_document` is either the scaled
    // root or a selected skin joint, so there is no attachment to probe.
    let bare = multi_joint_document();
    let bare_plan = multi_joint_plan(&bare, &capability);
    assert!(bare_plan.transform_only_attachments().is_empty());
    assert!(
        !bare_plan
            .obligations()
            .contains(&ScaleProofObligation::TransformOnlyAffine)
    );

    // The same operation on a closure that does carry one declares it,
    // so the negative above is the evidence gate rather than an
    // obligation that stopped being declared at all.
    let attached = compensated_document();
    let attached_plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &attached,
        capability: &capability,
    })
    .unwrap();
    assert_eq!(attached_plan.transform_only_attachments(), &[2]);
    assert!(
        attached_plan
            .obligations()
            .contains(&ScaleProofObligation::TransformOnlyAffine)
    );
}

#[test]
fn a_replayed_plan_cannot_reclassify_a_transform_only_attachment_as_a_joint() {
    let doc = compensated_document();
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    assert_eq!(plan.transform_only_attachments(), &[2]);

    // Claim the existing transform-only child as another joint of the
    // selected source skin. The closure still contains exactly the same
    // bones, but the off-origin affine obligation would disappear under
    // the stale plan's new source classification.
    let mut reclassified = doc.clone();
    let child_source = reclassified
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(2))
        .unwrap()
        .source_node_index;
    reclassified.assets.source_skeleton.skins[0]
        .joint_source_node_indices
        .push(child_source);
    assert_eq!(
        build_scale_candidate(&reclassified, &plan).unwrap_err(),
        ScaleError::PlanDocumentMismatch {
            reason: "affected_source_topology_mismatch"
        }
    );
}

#[test]
fn a_source_skin_whose_vertices_are_all_unweighted_names_the_missing_source_bounds() {
    // The instance still declares an affected joint, so the early
    // `has_skinned_evidence` gate is satisfied and this reaches the
    // `skinned_bounds` fallback. What is missing is a vertex that
    // actually binds to that joint: a fully unweighted vertex is
    // legitimately excluded from bounds, and with the fixture's only
    // vertex excluded the source yields no box at all.
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::SkinAndBounds)
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    prove_scale(&doc, &candidate, &plan).unwrap();

    let mut unweighted = doc.clone();
    unweighted.assets.meshes[0].primitives[0].weights[0] = [0.0; 4];
    assert_eq!(unweighted.assets.instances[0].skin_joints, vec![1]);
    let error = prove_scale(&unweighted, &candidate, &plan).unwrap_err();
    let ScaleError::MissingProofEvidence { kind, detail } = error else {
        panic!("expected missing bounds evidence, got {error:?}");
    };
    assert_eq!(kind, ProofResidualKind::Bounds);
    assert_eq!(detail, "source_bounds_missing");
}

#[test]
fn a_candidate_skin_whose_vertices_are_all_unweighted_names_the_missing_candidate_bounds() {
    // Same shape as the source case, on the other side of the comparison:
    // vertex weights are not part of the structural parity
    // `validate_candidate_structure` enforces, so a candidate can reach
    // the bounds obligation carrying a box-less skin while the source
    // still has one. The two sides must not be reported interchangeably.
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::SkinAndBounds)
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    let mut unweighted = candidate.document().clone();
    unweighted.assets.meshes[0].primitives[0].weights[0] = [0.0; 4];
    let unweighted = ScaleCandidate {
        document: unweighted,
    };
    let error = prove_scale(&doc, &unweighted, &plan).unwrap_err();
    let ScaleError::MissingProofEvidence { kind, detail } = error else {
        panic!("expected missing bounds evidence, got {error:?}");
    };
    assert_eq!(kind, ProofResidualKind::Bounds);
    assert_eq!(detail, "candidate_bounds_missing");
}

// --- Absent inverse-bind accessor through a rest/bind rebase ---------

#[test]
fn rest_bind_materializes_the_format_defined_identity_bind_it_must_conjugate() {
    // glTF's legal "skin omits `inverseBindMatrices`, every joint's bind
    // is identity" shape: an empty `skin_ibms` and no bone-level
    // convenience value. Rewriting only what is already stored touches
    // nothing here and silently emits `W' * B' = W * C * I != W * B`.
    let mut doc = compensated_document();
    doc.assets.instances[0].skin_ibms.clear();
    doc.assets.source_skeleton.skins[0].attachments = vec![SourceSkinAttachment {
        source_node_index: doc.assets.instances[0].source_node_index,
        source_mesh_index: Some(0),
    }];
    assert_eq!(
        doc.assets.source_skeleton.skins[0]
            .inverse_bind_accessor
            .status,
        SourceInverseBindAccessorStatus::Absent
    );
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    // `B' = C^-1 * B = scale(s) * I`, written explicitly so the
    // candidate describes its own bind rather than relying on a format
    // default that is no longer identity.
    let ibms = &candidate.document().assets.instances[0].skin_ibms;
    assert_eq!(ibms.len(), 1);
    assert!(ibms[0].abs_diff_eq(Mat4::from_scale(Vec3::splat(0.01)), 1e-8));
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert!(proof.skin_matrix.max() < 1e-6);
}

// --- Source-skeleton freshness and idempotence -----------------------

#[test]
fn rest_bind_rebases_the_raw_source_projection_alongside_the_skeleton() {
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let SourceNodeLocalRest::Trs { scale, .. } =
        &candidate.document().assets.source_skeleton.nodes[0].local_rest
    else {
        panic!("expected a trs source rest");
    };
    assert!((*scale - Vec3::ONE).length() < 1e-6);

    // The whole point: re-planning the candidate with the identical
    // request must not be accepted and double-apply the factor.
    let replanned = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: candidate.document(),
        capability: &capability,
    };
    assert!(matches!(
        plan_scale(&replanned).unwrap_err(),
        ScaleError::FactorMismatch { .. }
    ));
}

#[test]
fn proof_rejects_a_corrupt_direct_trs_source_rewrite_with_correct_normalized_bones() {
    let doc = compensated_document();
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    prove_scale(&doc, &candidate, &plan).unwrap();

    let mut adjacent = candidate.document().clone();
    let root = adjacent
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(0))
        .unwrap();
    let SourceNodeLocalRest::Trs { scale, .. } = &mut root.local_rest else {
        panic!("fixture root changed representation");
    };
    scale.x = f32::from_bits(scale.x.to_bits() + 1);
    prove_scale(&doc, &ScaleCandidate::from_document(adjacent), &plan)
        .expect("an adjacent-float TRS scale rewrite remains within tolerance");

    let mut corrupted = candidate.into_document();
    let root = corrupted
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(0))
        .unwrap();
    let SourceNodeLocalRest::Trs { scale, .. } = &mut root.local_rest else {
        panic!("fixture root changed representation");
    };
    scale.x += 0.01;
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate::from_document(corrupted), &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "field_disposition_mismatch"
        }
    );
}

/// The same compensated-scale relationship as [`compensated_document`],
/// with every affected node's authored local rest declared as
/// [`SourceNodeLocalRest::Matrix`] instead of `Trs` — the variant every
/// other fixture in this module leaves unexercised, and the only one that
/// reaches the reference writer's matrix-rebase path.
///
/// ```text
/// bone 0   parent -   scale(0.01)                   scaled root
/// bone 1   parent 0   T(0, 100, 0) * diag(-1,-1,1)  the skin's joint
/// bone 2   parent 1   T(0, 0, 50)                   transform-only child
/// ```
///
/// `diag(-1, -1, 1)` is the proper rotation by `pi` about z, so the
/// linear parts stay orthogonal with a positive determinant and the
/// domain classifies at `0.01`. The matching [`Bone::rest`] rotation is
/// `Quat::from_rotation_z(PI)`, whose `f32` matrix differs from that
/// literal by under `1e-7`.
///
/// Rest-world facts: bone 1 has linear `0.01 * diag(-1, -1, 1)` and
/// translation `(0, 1, 0)`; bone 2 adds `0.01 * (0, 0, 50)` for
/// `(0, 1, 0.5)`.
fn matrix_projection_document() -> Document {
    let nodes = vec![
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
            rotation: Quat::from_rotation_z(std::f32::consts::PI),
            scale: Vec3::ONE,
        },
        rig(Some(1), 2, Vec3::new(0.0, 0.0, 50.0)),
    ];
    // `B = inverse(W_rest(bone 1))`. With `R = diag(-1, -1, 1) = R^-1`,
    // `W = scale(0.01) * T(0, 100, 0) * R` has linear `0.01 * R` and
    // translation `(0, 1, 0)`, so `W^-1 = R * scale(100) * T(0, -1, 0)`:
    // linear `diag(-100, -100, 100)`, translation column `(0, 100, 0)`.
    let ibm = Mat4::from_cols(
        Vec4::new(-100.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -100.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 100.0, 0.0),
        Vec4::new(0.0, 100.0, 0.0, 1.0),
    );
    let mut doc = rig_document(&nodes, &[1], 0, ibm);
    let authored = [
        Mat4::from_cols(
            Vec4::new(0.01, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 0.01, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 0.01, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        ),
        Mat4::from_cols(
            Vec4::new(-1.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, -1.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 100.0, 0.0, 1.0),
        ),
        Mat4::from_cols(
            Vec4::new(1.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 0.0, 50.0, 1.0),
        ),
    ];
    for (node, matrix) in doc.assets.source_skeleton.nodes.iter_mut().zip(authored) {
        node.local_rest = SourceNodeLocalRest::Matrix(matrix);
    }
    doc
}

#[test]
fn rest_bind_rebases_a_matrix_declared_source_projection_to_agree_with_the_skeleton() {
    // `rebase_matrix` implements the `SourceNodeLocalRest::Matrix` half of
    // the source-projection rewrite and is unreachable from a `Trs`
    // fixture, so nothing else here executes it. The shipped code is
    // correct — this pins it against the same
    // `L' = scale(s_parent) * L * scale(1 / s_node)` the `Trs` half
    // applies, including the fact that a uniform right-multiply scales
    // the three linear columns and leaves the translation column alone.
    let doc = matrix_projection_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    assert_eq!(plan.transform_only_attachments(), &[2]);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // Hand-computed rewrites. Bone 0 is the scaled root, so
    // `s_parent = 1` and only its linear columns are divided by
    // `s = 0.01`: `scale(0.01) -> I`. Bones 1 and 2 have an affected
    // parent, so `s_parent = s_node = 0.01`: their linear columns are
    // multiplied and divided by the same factor and come out unchanged,
    // while their translation columns are multiplied by `0.01` alone —
    // `(0, 100, 0) -> (0, 1, 0)` and `(0, 0, 50) -> (0, 0, 0.5)`.
    let expected = [
        Mat4::IDENTITY,
        Mat4::from_cols(
            Vec4::new(-1.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, -1.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 1.0, 0.0, 1.0),
        ),
        Mat4::from_cols(
            Vec4::new(1.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 0.0, 0.5, 1.0),
        ),
    ];
    for (index, expected) in expected.into_iter().enumerate() {
        let SourceNodeLocalRest::Matrix(matrix) =
            &candidate.document().assets.source_skeleton.nodes[index].local_rest
        else {
            panic!("an authored matrix source rest must stay a matrix");
        };
        assert!(
            matrix.abs_diff_eq(expected, 1e-6),
            "source node {index} rebased to {matrix:?}"
        );
        // And the rewritten projection describes the same local transform
        // as the rewritten normalized bone: the two halves of the rest
        // rewrite must not drift apart.
        let rest = candidate.document().skeleton.bones[index].rest;
        let bone_matrix =
            Mat4::from_scale_rotation_translation(rest.scale, rest.rotation, rest.translation);
        assert!(
            matrix.abs_diff_eq(bone_matrix, 1e-6),
            "source node {index} projection {matrix:?} disagrees with bone rest {bone_matrix:?}"
        );
    }

    // `B' = C^-1 * B = scale(s) * B`: linear `diag(-1, -1, 1)`,
    // translation `(0, 1, 0)`.
    let binds = &candidate.document().assets.instances[0].skin_ibms;
    assert!(
        binds[0].abs_diff_eq(
            Mat4::from_cols(
                Vec4::new(-1.0, 0.0, 0.0, 0.0),
                Vec4::new(0.0, -1.0, 0.0, 0.0),
                Vec4::new(0.0, 0.0, 1.0, 0.0),
                Vec4::new(0.0, 1.0, 0.0, 1.0),
            ),
            1e-5
        ),
        "rebased bind {:?}",
        binds[0]
    );

    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert!(proof.rest_translation.max() < 1e-4);
    assert!(proof.rest_rotation.max() < 1e-9);
    assert!(proof.unit_scale.max() < 1e-4);
    assert!(proof.transform_only_affine.max() < 1e-4);
    assert!(proof.skin_matrix.max() < 1e-4);
}

#[test]
fn direct_matrix_rows_preserve_siblings_and_proof_checks_the_rewritten_translation() {
    let mut doc = matrix_projection_document();
    let translation = Vec3::new(3.25, -7.5, 11.0);
    let rotation = Quat::from_rotation_z(0.001_f32.to_radians());
    doc.skeleton.bones[1].rest.translation = translation;
    doc.skeleton.bones[1].rest.rotation = rotation;
    doc.assets.source_skeleton.nodes[1].local_rest = SourceNodeLocalRest::Matrix(
        Mat4::from_scale_rotation_translation(Vec3::ONE, rotation, translation),
    );
    let root = doc.skeleton.bones[0].rest;
    let joint = doc.skeleton.bones[1].rest;
    let joint_world =
        Mat4::from_scale_rotation_translation(root.scale, root.rotation, root.translation)
            * Mat4::from_scale_rotation_translation(joint.scale, joint.rotation, joint.translation);
    doc.assets.instances[0].skin_ibms[0] = joint_world.inverse();

    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    let SourceNodeLocalRest::Matrix(before) = &doc.assets.source_skeleton.nodes[1].local_rest
    else {
        panic!("fixture descendant changed representation");
    };
    let SourceNodeLocalRest::Matrix(after) =
        &candidate.document().assets.source_skeleton.nodes[1].local_rest
    else {
        panic!("candidate descendant changed representation");
    };

    // This nontrivial rotation makes the old whole-matrix rewrite
    // observable: multiplying a preserved linear component by `s_parent`
    // and then by `1 / s_node` changes at least one f32 bit even though
    // both factors describe the same basis. A field-row builder must
    // merge only the rewritten translation and retain these sibling bits.
    let old_two_step = |column: Vec4| {
        let scaled = Vec4::new(column.x * 0.01, column.y * 0.01, column.z * 0.01, column.w);
        scaled * (1.0 / 0.01)
    };
    assert_ne!(
        [
            old_two_step(before.x_axis),
            old_two_step(before.y_axis),
            old_two_step(before.z_axis),
        ]
        .map(|value| value.to_array().map(f32::to_bits)),
        [before.x_axis, before.y_axis, before.z_axis]
            .map(|value| value.to_array().map(f32::to_bits)),
        "fixture must expose the old two-step f32 linear roundtrip"
    );
    assert_eq!(
        [after.x_axis, after.y_axis, after.z_axis].map(|value| value.to_array().map(f32::to_bits)),
        [before.x_axis, before.y_axis, before.z_axis]
            .map(|value| value.to_array().map(f32::to_bits)),
        "a translation-only row must not perturb preserved linear columns"
    );
    assert_eq!(
        after.w_axis.truncate().to_array().map(f32::to_bits),
        (before.w_axis.truncate() * 0.01)
            .to_array()
            .map(f32::to_bits),
        "the direct row must rewrite exactly the translation component"
    );
    assert_eq!(
        [
            after.x_axis.w,
            after.y_axis.w,
            after.z_axis.w,
            after.w_axis.w,
        ]
        .map(f32::to_bits),
        [
            before.x_axis.w,
            before.y_axis.w,
            before.z_axis.w,
            before.w_axis.w,
        ]
        .map(f32::to_bits),
        "a translation-only row must preserve homogeneous components bit-exactly"
    );

    let mut adjacent_translation = candidate.document().clone();
    let SourceNodeLocalRest::Matrix(local) =
        &mut adjacent_translation.assets.source_skeleton.nodes[1].local_rest
    else {
        panic!("candidate descendant changed representation");
    };
    local.w_axis.y = f32::from_bits(local.w_axis.y.to_bits() + 1);
    prove_scale(
        &doc,
        &ScaleCandidate::from_document(adjacent_translation),
        &plan,
    )
    .expect("an adjacent-float matrix translation rewrite remains within tolerance");

    let mut adjacent_linear = candidate.document().clone();
    let SourceNodeLocalRest::Matrix(local) =
        &mut adjacent_linear.assets.source_skeleton.nodes[0].local_rest
    else {
        panic!("candidate root changed representation");
    };
    local.x_axis.x = f32::from_bits(local.x_axis.x.to_bits() + 1);
    prove_scale(&doc, &ScaleCandidate::from_document(adjacent_linear), &plan)
        .expect("an adjacent-float matrix linear rewrite remains within tolerance");

    let mut corrupt_linear = candidate.document().clone();
    let SourceNodeLocalRest::Matrix(local) =
        &mut corrupt_linear.assets.source_skeleton.nodes[0].local_rest
    else {
        panic!("candidate root changed representation");
    };
    local.x_axis.x += 0.1;
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate::from_document(corrupt_linear), &plan,).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "field_disposition_mismatch"
        }
    );

    let mut corrupted = candidate.into_document();
    let SourceNodeLocalRest::Matrix(local) =
        &mut corrupted.assets.source_skeleton.nodes[1].local_rest
    else {
        panic!("candidate descendant changed representation");
    };
    local.w_axis.y += 1.0;
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate::from_document(corrupted), &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "field_disposition_mismatch"
        }
    );
}

#[test]
fn whole_document_conversion_rebases_the_raw_source_projection_too() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let SourceNodeLocalRest::Trs {
        translation, scale, ..
    } = &candidate.document().assets.source_skeleton.nodes[1].local_rest
    else {
        panic!("expected a trs source rest");
    };
    assert!((*translation - Vec3::new(0.0, 0.01, 0.0)).length() < 1e-8);
    // Dimensionless: a linear-unit conversion never touches it.
    assert_eq!(*scale, Vec3::ONE);
}

#[test]
fn whole_document_proof_checks_raw_rewrites_without_using_normalized_bones_as_a_proxy() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut adjacent = candidate.document().clone();
    let SourceNodeLocalRest::Trs { translation, .. } =
        &mut adjacent.assets.source_skeleton.nodes[1].local_rest
    else {
        panic!("fixture source rest changed representation");
    };
    translation.y = f32::from_bits(translation.y.to_bits() + 1);
    prove_scale(&doc, &ScaleCandidate::from_document(adjacent), &plan)
        .expect("an adjacent-float raw rewrite remains within the published tolerance");

    let mut corrupted = candidate.into_document();
    let SourceNodeLocalRest::Trs { translation, .. } =
        &mut corrupted.assets.source_skeleton.nodes[1].local_rest
    else {
        panic!("fixture source rest changed representation");
    };
    translation.y += 1.0;
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate::from_document(corrupted), &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "field_disposition_mismatch"
        }
    );
}

#[test]
fn unavailable_source_rows_are_converted_but_never_become_replay_identity() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let mut matrix = Mat4::from_rotation_z(0.37);
    matrix.w_axis = Vec4::new(3.25, -7.5, 11.0, 1.0);
    doc.assets.source_skeleton.nodes.push(SourceNodeAsset::new(
        77,
        SourceNodeLocalRest::Matrix(matrix),
    ));
    doc.assets.source_skeleton.coverage = SourceSkeletonCoverage::Unavailable;
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert_eq!(plan.ledger().source_topology().count(), 0);
    assert!(
        !plan
            .ledger()
            .field_rows()
            .any(|row| matches!(row.target(), ScaleFieldTarget::SourceNodeRest { .. }))
    );

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let SourceNodeLocalRest::Trs { translation, .. } =
        &candidate.document().assets.source_skeleton.nodes[1].local_rest
    else {
        panic!("fixture source rest changed representation");
    };
    assert_eq!(*translation, Vec3::new(0.0, 0.01, 0.0));
    let SourceNodeLocalRest::Matrix(converted_matrix) =
        &candidate.document().assets.source_skeleton.nodes[2].local_rest
    else {
        panic!("best-effort matrix source rest changed representation");
    };
    assert_eq!(
        converted_matrix
            .w_axis
            .truncate()
            .to_array()
            .map(f32::to_bits),
        (matrix.w_axis.truncate() * 0.01)
            .to_array()
            .map(f32::to_bits),
        "Unavailable matrix translation must still receive unit conversion"
    );
    assert_eq!(
        [
            converted_matrix.x_axis,
            converted_matrix.y_axis,
            converted_matrix.z_axis,
        ]
        .map(|value| value.to_array().map(f32::to_bits)),
        [matrix.x_axis, matrix.y_axis, matrix.z_axis]
            .map(|value| value.to_array().map(f32::to_bits)),
        "Unavailable matrix linear columns remain dimensionless"
    );
    assert_eq!(
        [
            converted_matrix.x_axis.w,
            converted_matrix.y_axis.w,
            converted_matrix.z_axis.w,
            converted_matrix.w_axis.w,
        ]
        .map(f32::to_bits),
        [
            matrix.x_axis.w,
            matrix.y_axis.w,
            matrix.z_axis.w,
            matrix.w_axis.w,
        ]
        .map(f32::to_bits),
        "Unavailable matrix homogeneous components remain bit-exact"
    );

    let mut replay = doc.clone();
    replay.assets.source_skeleton.nodes[1].source_node_index = 123_456;
    replay.assets.source_skeleton.nodes[2].source_node_index = 654_321;
    let replayed = build_scale_candidate(&replay, &plan).unwrap();
    let SourceNodeLocalRest::Trs { translation, .. } =
        &replayed.document().assets.source_skeleton.nodes[1].local_rest
    else {
        panic!("fixture source rest changed representation");
    };
    assert_eq!(*translation, Vec3::new(0.0, 0.01, 0.0));
    let SourceNodeLocalRest::Matrix(replayed_matrix) =
        &replayed.document().assets.source_skeleton.nodes[2].local_rest
    else {
        panic!("replayed best-effort matrix source rest changed representation");
    };
    assert_eq!(
        replayed_matrix.to_cols_array().map(f32::to_bits),
        converted_matrix.to_cols_array().map(f32::to_bits),
        "Unavailable matrix conversion must ignore non-authoritative raw identity"
    );
}

// --- Per-obligation falsifiability ----------------------------------
//
// DESIGN.md Appendix D §D.6 lists the claims proof must establish as
// *separate* obligations. A suite in which every doctored candidate is
// caught by whichever obligation happens to run first proves only that
// *something* is checked, so each test below is built around a candidate
// whose single defect is visible to one obligation and is asserted to
// name exactly that [`ProofResidualKind`]. Turning the matching
// obligation off is what must make each of these tests fail.

/// Root, one skinned joint, and a leaf attachment carrying no skin, no
/// mesh instance and no animation track: the only obligation that can
/// observe that leaf at all is the rest-world translation check.
fn rest_only_leaf_rig() -> Vec<RigNode> {
    vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        rig(Some(0), 2, Vec3::new(3.0, 0.0, 0.0)),
    ]
}

#[test]
fn an_un_rewritten_rest_translation_is_named_by_the_rest_translation_obligation() {
    let doc = rig_document(&rest_only_leaf_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    // Analytic expectation: `(3, 0, 0) * 0.01`.
    assert!(
        (candidate.document().skeleton.bones[2].rest.translation - Vec3::new(0.03, 0.0, 0.0))
            .length()
            < 1e-8
    );
    let mut broken = candidate.document().clone();
    // A builder that skipped exactly one node's rest translation. The
    // leaf carries no skin slot, no mesh, and no track, so no sampled,
    // skin, or bounds obligation can see it.
    broken.skeleton.bones[2].rest.translation = Vec3::new(3.0, 0.0, 0.0);
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::RestTranslation,
            ..
        }
    ));
}

#[test]
fn a_rest_translation_error_confined_to_z_is_still_named_by_the_rest_translation_obligation() {
    // The rest-translation residual is a three-component length, and
    // every other fixture's translation error has an x or y term, so a
    // residual that quietly dropped its z term would still be caught
    // everywhere else. This candidate keeps x and y at the analytically
    // expected `(3, 0, 0) * 0.01` and moves z alone; as in the test
    // above, the leaf carries no skin slot, mesh vertex or track, so the
    // rest-translation obligation is the only one that can see it at all.
    let doc = rig_document(&rest_only_leaf_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    broken.skeleton.bones[2].rest.translation = Vec3::new(0.03, 0.0, 1.0);
    let broken = ScaleCandidate { document: broken };
    let error = prove_scale(&doc, &broken, &plan).unwrap_err();
    assert!(
        matches!(
            error,
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::RestTranslation,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_non_unit_composed_scale_on_an_affected_node_is_named_by_the_unit_scale_obligation() {
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    // A leaf's own local scale does not move its own world origin and is
    // not the `rest.rotation` field the rotation obligation compares, so
    // the postcondition "unit composed scale for every affected node" is
    // the first obligation that can see it. Composed world scale becomes
    // `(2, 2, 2)`: a residual of `sqrt(3)` against a `1e-5` policy.
    broken.skeleton.bones[2].rest.scale = Vec3::splat(2.0);
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::UnitScale,
            ..
        }
    ));
}

#[test]
fn a_composed_scale_anomaly_confined_to_z_is_still_named_by_the_unit_scale_obligation() {
    // The postcondition residual sums a per-axis deviation from one, and
    // no other fixture puts a composed-scale anomaly on z alone, so a
    // residual that dropped its z axis would still be caught everywhere
    // else. Here the composed x and y scales stay one and only z becomes
    // two: dropping z reports `0.0` and hands the candidate on to a
    // *different* obligation, which is why this test names the kind
    // rather than merely asserting a rejection.
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    broken.skeleton.bones[2].rest.scale = Vec3::new(1.0, 1.0, 2.0);
    let broken = ScaleCandidate { document: broken };
    let error = prove_scale(&doc, &broken, &plan).unwrap_err();
    assert!(
        matches!(
            error,
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::UnitScale,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_transform_only_attachment_with_a_correct_origin_but_a_wrong_linear_part_still_fails() {
    // DESIGN.md Appendix D §D.6 requires proving "the analytically
    // expected full world affine of a transform-only attached child ...
    // so a no-op cannot pass". The existing stale-attachment fixture is
    // already caught by the rest-*translation* check, which proves
    // nothing about the linear part. This candidate keeps the
    // attachment's world origin exactly right and its composed scale
    // exactly one — so `RestTranslation`, `RestRotation` and `UnitScale`
    // all pass — while flipping two axes of its linear part. Only
    // transforming an off-origin point through the complete affine can
    // see it.
    //
    // A *magnitude* error in the linear part would be caught first by
    // the unit-scale postcondition; `diag(-1, -1, 1)` is a proper
    // rotation by pi about z, so every column length stays one and the
    // determinant stays positive.
    let doc = compensated_document();
    let capability = complete_capability();
    let request = ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    };
    let plan = plan_scale(&request).unwrap();
    assert_eq!(plan.transform_only_attachments(), &[2]);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    broken.skeleton.bones[2].rest.scale = Vec3::new(-1.0, -1.0, 1.0);
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::TransformOnlyAffine,
            ..
        }
    ));
}

#[test]
fn an_inverse_bind_whose_linear_part_was_not_conjugated_is_named_by_the_skin_obligation() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    // The single skinned vertex sits at the geometry origin, so
    // `(W * B) * p` reduces to the translation column of `W * B` and the
    // bounds obligation is analytically blind to a change confined to
    // `B`'s linear part.
    doc.assets.meshes[0].primitives[0].positions[0] = Vec3::ZERO;
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    broken.assets.instances[0].skin_ibms[0].x_axis = Vec4::new(2.0, 0.0, 0.0, 0.0);
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::SkinMatrix,
            ..
        }
    ));
}

#[test]
fn a_rest_scale_that_only_shows_up_under_animation_is_named_by_the_trajectory_obligation() {
    // Bone 0 is the only skin joint; bones 1 and 2 are transform-only
    // descendants whose *rest* translations are both zero, so doctoring
    // bone 1's rest scale moves nothing at rest, nothing in the skin
    // equation, and nothing in any stored track value — it only shows up
    // once bone 2's translation track drives it off its parent's origin.
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::ZERO),
        rig(Some(1), 2, Vec3::ZERO),
    ];
    let mut doc = rig_document(&nodes, &[0], 0, Mat4::IDENTITY);
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 2,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)]),
        }],
    });
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    broken.skeleton.bones[1].rest.scale = Vec3::splat(2.0);
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::Trajectory,
            ..
        }
    ));
}

/// A `CUBICSPLINE` translation segment whose two key values are both the
/// origin and whose out/in tangents are equal and large.
///
/// The two properties this fixture exists for are analytic:
///
/// * at either key time the sampled value is exactly the key value, so a
///   *tangent* perturbation is invisible to the key-time obligations;
/// * at the segment midpoint the Hermite basis contributes
///   `h10 * dt * m0 + h11 * dt * m1 = 0.125 * m0 - 0.125 * m1`, which is
///   exactly zero while `m0 == m1` — so a perturbation `d` of one
///   tangent moves the sampled midpoint by `0.125 * d` away from a zero
///   expectation, where the policy tolerance is only `1e-6`.
///
/// A tangent magnitude of `1000` makes the *element-wise* `TrackValue`
/// tolerance `1e-6 + 1e-5 * 1000 = 1.0001e-2`, so a `1e-3` perturbation
/// is comfortably inside it and comfortably outside the `1.25e-4`
/// midpoint residual it produces. That gap is what lets these two tests
/// isolate the sampled obligations from the element-wise one.
fn flat_cubic_translation_track() -> Track {
    Track {
        bone: 1,
        property: Property::Translation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![
            Vec3::ZERO,                  // in-tangent @0
            Vec3::ZERO,                  // value @0
            Vec3::new(0.0, 1000.0, 0.0), // out-tangent @0 (`m0`)
            Vec3::new(0.0, 1000.0, 0.0), // in-tangent @1 (`m1`)
            Vec3::ZERO,                  // value @1
            Vec3::ZERO,                  // out-tangent @1
        ]),
    }
}

fn identity_conversion_plan(document: &Document, capability: &ScaleCapabilityFacts) -> ScalePlan {
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.0 },
        document,
        capability,
    })
    .unwrap()
}

#[test]
fn a_cubic_tangent_error_inside_element_tolerance_is_named_by_the_cubic_interior_obligation() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![flat_cubic_translation_track()],
    });
    let capability = complete_capability();
    let plan = identity_conversion_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    let TrackValues::Vec3s(values) = &mut broken.clips[0].tracks[0].values else {
        panic!("expected a vec3 track");
    };
    values[2].y = 1000.001;
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::CubicInterior,
            ..
        }
    ));
}

#[test]
fn the_same_tangent_error_at_a_harvested_key_time_is_named_by_the_key_obligation() {
    // Identical defect to the test above, but a rotation track keyed at
    // `0.5` promotes the segment midpoint to a *key* time. Key times are
    // proved before cubic interiors, so this candidate is named by
    // `KeyTranslation` — which is otherwise unreachable, since at a
    // segment's own key times the sampled value is the stored value the
    // element-wise `TrackValue` check already owns.
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![
            flat_cubic_translation_track(),
            Track {
                bone: 1,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.5],
                values: TrackValues::Quats(vec![Quat::from_rotation_y(0.3)]),
            },
        ],
    });
    let capability = complete_capability();
    let plan = identity_conversion_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    let TrackValues::Vec3s(values) = &mut broken.clips[0].tracks[0].values else {
        panic!("expected a vec3 track");
    };
    values[2].y = 1000.001;
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::KeyTranslation,
            ..
        }
    ));
}

// --- Multi-joint, multi-vertex, animated rest/bind fixture ------------

/// The multi-joint half of DESIGN.md Appendix D §D.6's "analytic
/// one-joint **and multi-joint** fixtures", with the §D.4 translation
/// animation domain actually populated.
///
/// Two compensated joints (`0.01` at every rest-world linear part), four
/// vertices — two singly weighted, one genuinely blended `0.25 / 0.75`,
/// and one whose weights sum to `0.8` so the normalisation step in
/// [`accumulate_skinned_bounds`] is exercised — plus a `LINEAR` translation track
/// on one joint and a `CUBICSPLINE` translation track (with non-zero
/// tangents) on the other, so key, cubic-interior, trajectory, skin and
/// bounds obligations all have something to evaluate.
///
/// Analytic facts, all hand-derived:
///
/// ```text
/// W1(rest) = [0.01 I | (0, 1, 0)]   B1 = [100 I | (0, -100, 0)]
/// W2(rest) = [0.01 I | (0, 2, 0)]   B2 = [100 I | (0, -200, 0)]
/// ```
///
/// so `W_i(rest) * B_i == I` for both joints (geometry bind `G = I`).
fn multi_joint_document() -> Document {
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
        rig(Some(1), 2, Vec3::new(0.0, 100.0, 0.0)),
    ];
    let mut doc = rig_document(&nodes, &[1, 2], 0, Mat4::IDENTITY);
    doc.assets.instances[0].skin_ibms = vec![
        Mat4::from_scale_rotation_translation(
            Vec3::splat(100.0),
            Quat::IDENTITY,
            Vec3::new(0.0, -100.0, 0.0),
        ),
        Mat4::from_scale_rotation_translation(
            Vec3::splat(100.0),
            Quat::IDENTITY,
            Vec3::new(0.0, -200.0, 0.0),
        ),
    ];
    doc.assets.meshes[0].primitives[0] = Primitive {
        positions: vec![
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 2.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, -3.0, 0.0),
        ],
        joints: vec![[0, 0, 0, 0], [1, 0, 0, 0], [0, 1, 0, 0], [0, 1, 0, 0]],
        weights: vec![
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.25, 0.75, 0.0, 0.0],
            // Deliberately sums to `0.8`, not `1.0`.
            [0.4, 0.4, 0.0, 0.0],
        ],
        ..Primitive::default()
    };
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![
            Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::new(0.0, 100.0, 0.0),
                    Vec3::new(0.0, 200.0, 0.0),
                ]),
            },
            Track {
                bone: 2,
                property: Property::Translation,
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::ZERO,                 // in-tangent @0
                    Vec3::new(0.0, 100.0, 0.0), // value @0
                    Vec3::new(0.0, 60.0, 0.0),  // out-tangent @0
                    Vec3::new(0.0, 60.0, 0.0),  // in-tangent @1
                    Vec3::new(0.0, 300.0, 0.0), // value @1
                    Vec3::ZERO,                 // out-tangent @1
                ]),
            },
        ],
    });
    doc
}

fn multi_joint_plan(document: &Document, capability: &ScaleCapabilityFacts) -> ScalePlan {
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document,
        capability,
    })
    .unwrap()
}

#[test]
fn skinned_bounds_blends_and_normalises_multi_joint_weights() {
    // Hand-written joint palettes, so nothing here is recomputed by the
    // code under test. At these poses `multi_joint_document`'s two skin
    // slots compose to
    //
    //     M1 = W1 * B1 = [0.01 I | (0, 2, 0)] * [100 I | (0, -100, 0)]
    //                  = [I | 0.01 * (0, -100, 0) + (0, 2, 0)]
    //                  = [I | (0, 1, 0)]
    //     M2 = W2 * B2 = [0.01 I | (0, 5, 0)] * [100 I | (0, -200, 0)]
    //                  = [I | 0.01 * (0, -200, 0) + (0, 5, 0)]
    //                  = [I | (0, 3, 0)]
    //
    // and the four vertices skin to
    //
    //     (1, 0, 0)  -> M1                                 = ( 1,  1, 0)
    //     (-1, 0, 2) -> M2                                 = (-1,  3, 2)
    //     (0, 2, 0)  -> 0.25 * (0, 3, 0) + 0.75 * (0, 5, 0) = ( 0, 4.5, 0)
    //     (0, -3, 0) -> (0.4 * (0, -2, 0) + 0.4 * (0, 0, 0)) / 0.8
    //                                                       = ( 0, -1, 0)
    let doc = multi_joint_document();
    let slots = [
        unrounded_slot(Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0))),
        unrounded_slot(Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0))),
    ];
    let mut accumulator = BoundsAccumulator::default();
    accumulate_skinned_bounds(
        0,
        0,
        &doc.assets.meshes[0].primitives[0],
        &slots,
        &mut accumulator,
    )
    .unwrap();
    let (min, max) = accumulator
        .finish()
        .expect("multi-joint fixture has weighted vertices");
    assert!(
        (min - Vec3::new(-1.0, -1.0, 0.0)).length() < 1e-5,
        "min {min:?}"
    );
    assert!(
        (max - Vec3::new(1.0, 4.5, 2.0)).length() < 1e-5,
        "max {max:?}"
    );
}

/// A skin slot whose composition is the matrix itself: `M * I` rounds
/// nothing, and it is given an empty parent chain, so these fixtures
/// assert about the bound extremes without a rounding magnitude in the
/// way.
fn unrounded_slot(matrix: Mat4) -> SkinSlot {
    SkinSlot::compose(matrix, Mat4::IDENTITY, 0.0)
}

/// One vertex, one slot, one influence — the smallest primitive that
/// exercises [`accumulate_skinned_bounds`]'s per-influence path.
fn one_influence_primitive(weight: f32) -> Primitive {
    Primitive {
        positions: vec![Vec3::new(1.0, 0.0, 0.0)],
        joints: vec![[0, 0, 0, 0]],
        weights: vec![[weight, 0.0, 0.0, 0.0]],
        ..Primitive::default()
    }
}

#[test]
fn an_infinite_skin_weight_is_rejected_as_a_weight_not_as_a_skinned_result() {
    // The guard is `!weight.is_finite()`, not `weight.is_nan()`, and the
    // two differ exactly on the infinities. An infinite weight is not
    // merely a `NaN` in disguise: it survives `weight != 0.0`, and
    // `inf * (1, 0, 0)` is `(inf, NaN, NaN)` because `inf * 0` is `NaN`,
    // so a `NaN`-only guard would let the vertex through and the failure
    // would surface downstream as `non_finite_result` — blaming the skin
    // equation for a malformed input attribute. The reason string is the
    // assertion, not just the fact of an error.
    let slots = [unrounded_slot(Mat4::IDENTITY)];
    let mut accumulator = BoundsAccumulator::default();
    assert_eq!(
        accumulate_skinned_bounds(
            7,
            3,
            &one_influence_primitive(f32::INFINITY),
            &slots,
            &mut accumulator,
        ),
        Err(ScaleError::InvalidSkinnedPrimitive {
            instance_index: 7,
            primitive_index: 3,
            reason: "non_finite_weight",
        })
    );
    // `NaN` names the same reason, so the guard is one guard and not two
    // behaviours that happen to share a spelling.
    assert_eq!(
        accumulate_skinned_bounds(
            7,
            3,
            &one_influence_primitive(f32::NAN),
            &slots,
            &mut accumulator,
        ),
        Err(ScaleError::InvalidSkinnedPrimitive {
            instance_index: 7,
            primitive_index: 3,
            reason: "non_finite_weight",
        })
    );
}

#[test]
fn bounds_provenance_is_invariant_when_weights_are_rescaled_to_subnormals() {
    // At p = (0.5, 0.5, 0.5), the identity transform's operand magnitude
    // is 1, so these distinct slot magnitudes dominate and are the exact
    // bases. The ordinary tuple sums to `0.75`, pinning the denominator in
    // `sum(w_i * base_i) / sum(w_i)`; the subnormal tuple is an exact
    // uniform rescaling and must retain both the point and provenance.
    let slots = [
        SkinSlot {
            matrix: Mat4::IDENTITY,
            absolute: Mat4::IDENTITY,
            rounding_magnitude: 6.0,
        },
        SkinSlot {
            matrix: Mat4::IDENTITY,
            absolute: Mat4::IDENTITY,
            rounding_magnitude: 3.0,
        },
    ];
    let measure = |weights: [f32; 4]| {
        let primitive = Primitive {
            positions: vec![Vec3::splat(0.5)],
            joints: vec![[0, 1, 0, 0]],
            weights: vec![weights],
            ..Primitive::default()
        };
        let mut accumulator = BoundsAccumulator::default();
        accumulate_skinned_bounds(0, 0, &primitive, &slots, &mut accumulator).unwrap();
        let magnitude = accumulator.rounding_magnitude();
        (accumulator.finish(), magnitude)
    };

    let tiny = f32::from_bits(1);
    let two_tiny = f32::from_bits(2);
    let scale = f32::from_bits(4);
    assert!(tiny.is_subnormal() && two_tiny.is_subnormal());
    assert_eq!(0.25 * scale, tiny);
    assert_eq!(0.5 * scale, two_tiny);

    let ordinary = measure([0.25, 0.5, 0.0, 0.0]);
    let subnormal = measure([tiny, two_tiny, 0.0, 0.0]);
    assert_eq!(ordinary, (Some((Vec3::splat(0.5), Vec3::splat(0.5))), 4.0));
    assert_eq!(subnormal, ordinary);
}

#[test]
fn weight_normalization_does_not_overflow_at_f32_max() {
    // Two maximum finite weights overflow a binary32 denominator, but
    // still describe the same normalized identity blend at `.5`.
    assert!((f32::MAX + f32::MAX).is_infinite());
    let primitive = Primitive {
        positions: vec![Vec3::splat(0.5)],
        joints: vec![[0, 0, 0, 0]],
        weights: vec![[f32::MAX, f32::MAX, 0.0, 0.0]],
        ..Primitive::default()
    };
    let mut accumulator = BoundsAccumulator::default();
    accumulate_skinned_bounds(
        7,
        3,
        &primitive,
        &[unrounded_slot(Mat4::IDENTITY)],
        &mut accumulator,
    )
    .unwrap();
    let (min, max) = accumulator.finish().unwrap();
    assert_eq!(min, Vec3::splat(0.5));
    assert_eq!(max, Vec3::splat(0.5));
}

#[test]
fn widened_weight_accumulation_stays_convex_at_f32_max() {
    // Even precomputed binary32 coefficients are insufficient. These
    // exact dyadic weights round to coefficients whose exact represented
    // sum is greater than one, so applying them to `f32::MAX` overflows an
    // identity blend that must remain exactly at its finite endpoint.
    let stored: [f32; 2] = [19.0 / 64.0, 9.0 / 64.0];
    let weight_scale = stored[0].max(stored[1]);
    let scaled = [stored[0] / weight_scale, stored[1] / weight_scale];
    let scaled_sum = scaled[0] + scaled[1];
    let coefficients = [scaled[0] / scaled_sum, scaled[1] / scaled_sum];
    assert!(f64::from(coefficients[0]) + f64::from(coefficients[1]) > 1.0);

    let position = Vec3::splat(f32::MAX);
    assert!(position.is_finite());
    assert!(
        !(coefficients[0] * position + coefficients[1] * position).is_finite(),
        "the binary32 counterexample no longer overflows"
    );
    let primitive = Primitive {
        positions: vec![position],
        joints: vec![[0, 1, 0, 0]],
        weights: vec![[stored[0], stored[1], 0.0, 0.0]],
        ..Primitive::default()
    };
    let mut accumulator = BoundsAccumulator::default();
    accumulate_skinned_bounds(
        7,
        3,
        &primitive,
        &[
            unrounded_slot(Mat4::IDENTITY),
            unrounded_slot(Mat4::IDENTITY),
        ],
        &mut accumulator,
    )
    .unwrap();

    let (min, max) = accumulator.finish().unwrap();
    assert_eq!(min, position);
    assert_eq!(max, position);
}

#[test]
fn every_public_scale_boundary_rejects_a_negative_skin_weight() {
    let doc = multi_joint_document();
    let capability = complete_capability();
    let operations = [
        ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
    ];
    let mut signed = doc.clone();
    signed.assets.meshes[0].primitives[0].weights[3][3] = -f32::from_bits(1);
    let expected = ScaleError::NegativeSkinWeight {
        mesh_index: 0,
        primitive_index: 0,
        vertex_index: 3,
        influence_index: 3,
    };

    for operation in operations {
        let plan = plan_scale(&ScaleRequest {
            operation,
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        assert_eq!(
            plan_scale(&ScaleRequest {
                operation,
                document: &signed,
                capability: &capability,
            })
            .unwrap_err(),
            expected
        );
        assert_eq!(build_scale_candidate(&signed, &plan).unwrap_err(), expected);
        assert_eq!(
            prove_scale(&signed, &candidate, &plan).unwrap_err(),
            expected
        );
        assert_eq!(
            prove_scale(&doc, &ScaleCandidate::from_document(signed.clone()), &plan).unwrap_err(),
            expected
        );
    }

    // IEEE negative zero is numerically zero, not a negative influence.
    // Pin it across the same public boundaries so an implementation based
    // on `is_sign_negative()` cannot narrow the accepted domain.
    let mut negative_zero = doc.clone();
    negative_zero.assets.meshes[0].primitives[0].weights[3][3] = -0.0;
    for operation in operations {
        let plan = plan_scale(&ScaleRequest {
            operation,
            document: &negative_zero,
            capability: &capability,
        })
        .expect("negative zero is a zero skin influence");
        let candidate = build_scale_candidate(&negative_zero, &plan)
            .expect("candidate construction accepts negative zero");
        prove_scale(&negative_zero, &candidate, &plan)
            .expect("proof accepts negative zero in source and candidate");
    }

    // This PR owns finite-negative classification only. Non-finite
    // weights retain their existing bounds-path reason instead of being
    // reclassified merely because negative infinity compares below zero.
    let mut non_finite = doc.clone();
    non_finite.assets.meshes[0].primitives[0].weights[3][3] = f32::NEG_INFINITY;
    for operation in operations {
        let plan = plan_scale(&ScaleRequest {
            operation,
            document: &non_finite,
            capability: &capability,
        })
        .expect("negative infinity is not a finite-negative plan refusal");
        let candidate = build_scale_candidate(&non_finite, &plan)
            .expect("candidate construction preserves non-finite routing");
        assert!(matches!(
            prove_scale(&non_finite, &candidate, &plan),
            Err(ScaleError::InvalidSkinnedPrimitive {
                reason: "non_finite_weight",
                ..
            })
        ));
    }
}

#[test]
fn representative_finite_negative_weights_are_all_refused() {
    let doc = multi_joint_document();
    let capability = complete_capability();
    for weight in [
        -f32::from_bits(1),
        -f32::MIN_POSITIVE,
        -0.1,
        -0.25,
        -f32::MAX,
    ] {
        let mut signed = doc.clone();
        signed.assets.meshes[0].primitives[0].weights[0][0] = weight;
        assert!(matches!(
            plan_scale(&ScaleRequest {
                operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
                document: &signed,
                capability: &capability,
            }),
            Err(ScaleError::NegativeSkinWeight { .. })
        ));
    }
}

#[test]
fn a_negative_weight_reports_its_nonzero_mesh_and_primitive_location() {
    let mut doc = multi_joint_document();
    doc.assets.meshes.push(MeshAsset {
        name: "later mesh".into(),
        // Deliberately differs from this asset's vector index (`1`): the
        // typed error reports the normalized mesh-array coordinate, not
        // the source format's stable mesh id.
        source_mesh_index: 99,
        primitives: vec![
            Primitive::default(),
            Primitive::default(),
            Primitive {
                positions: vec![Vec3::ZERO],
                joints: vec![[0, 0, 0, 0]],
                weights: vec![[0.0, 0.0, -f32::from_bits(1), 0.0]],
                ..Primitive::default()
            },
        ],
    });
    let capability = complete_capability();
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
            document: &doc,
            capability: &capability,
        })
        .unwrap_err(),
        ScaleError::NegativeSkinWeight {
            mesh_index: 1,
            primitive_index: 2,
            vertex_index: 0,
            influence_index: 2,
        }
    );
}

// --- f32 rounding under rotation (`f32_rounding_ulps`) ---------------

/// A rotating skinned rig: a uniformly scaled root, two rotated joints in
/// a chain below it, and one primitive whose vertices bind to both.
///
/// Every rotation these fixtures use is a *literal*, never a derived or
/// sampled one. The rotation is the load-bearing parameter of every
/// defect in this section — it is what makes a bound component, or a
/// skin matrix entry, orders of magnitude smaller than the operands its
/// rounding error came from — so a fixture that leaves it implicit
/// records a factor and a point that on their own reproduce nothing.
///
/// `skin_ibms` is the analytic inverse of each joint's rest-world matrix,
/// so `W * B` is the identity in exact arithmetic and every residual
/// these fixtures measure is `f32` rounding and nothing else.
fn rotating_rig_document(
    rotations: [Quat; 2],
    factor: f32,
    locals: [Vec3; 2],
    points: &[Vec3],
    weights: &[[f32; 4]],
) -> Document {
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(factor),
        },
        RigNode {
            parent: Some(0),
            source_node_index: 1,
            translation: locals[0],
            rotation: rotations[0],
            scale: Vec3::ONE,
        },
        RigNode {
            parent: Some(1),
            source_node_index: 2,
            translation: locals[1],
            rotation: rotations[1],
            scale: Vec3::ONE,
        },
    ];
    let root = Mat4::from_scale(Vec3::splat(factor));
    let first = root * Mat4::from_rotation_translation(rotations[0], locals[0]);
    let second = first * Mat4::from_rotation_translation(rotations[1], locals[1]);
    let mut doc = rig_document(&nodes, &[1, 2], 0, Mat4::IDENTITY);
    doc.assets.instances[0].skin_ibms = vec![first.inverse(), second.inverse()];
    let primitive = &mut doc.assets.meshes[0].primitives[0];
    primitive.positions = points.to_vec();
    primitive.joints = vec![[0, 1, 0, 0]; points.len()];
    primitive.weights = weights.to_vec();
    doc
}

fn rest_bind_plan(document: &Document, expected_factor: f64) -> ScalePlan {
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor,
        },
        document,
        capability: &complete_capability(),
    })
    .expect("the rotating rig plans at its own observed factor")
}

/// The rotation, factor and point of the reproducer, as literals.
fn reproducer_document() -> Document {
    rotating_rig_document(
        [
            Quat::from_xyzw(-0.81788284, 0.343121, -0.45392478, -0.085369624),
            Quat::from_xyzw(-0.12301501, 0.043325406, -0.015209139, 0.991342),
        ],
        3190.0,
        [Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.3, 0.4, 0.5)],
        &[
            Vec3::new(2827.01, -5.982, 3162.68),
            Vec3::new(-1000.0, 7.5, -2000.0),
        ],
        &[[1.0, 0.0, 0.0, 0.0], [0.5, 0.5, 0.0, 0.0]],
    )
}

#[test]
fn a_rotated_rig_proves_a_correct_candidate_whose_bound_component_is_small() {
    // The reproducer. Under this rotation the skinned point
    // `(2827.01, -5.982, 3162.68)` has magnitude `4242` and a `y` of
    // `-5.98`, and the `y` bound carries the whole vector's rounding
    // error: the measured residual is `2.44e-4`, against a purely
    // per-axis tolerance of `1e-6 + 1e-5 * 5.98 = 6.08e-5`. The candidate
    // is correct — `build_scale_candidate` produced it — so refusing it
    // is a false negative, and `plan_scale` had already accepted the plan
    // it was built from.
    let doc = reproducer_document();
    let plan = rest_bind_plan(&doc, 3190.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan)
        .expect("a correct candidate under rotation must prove");

    // The residual really is above the per-axis-only band, so this
    // fixture is exercising the new term rather than passing for want of
    // a defect. `2.44e-4` is `2^-12`, one ulp of `2048`.
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    assert!(
        proof.bounds.max() > policy.scalar_tolerance(5.982, 5.982),
        "bounds residual {} no longer exceeds the per-axis band",
        proof.bounds.max()
    );
    assert!(
        proof.skin_matrix.max() > policy.scalar_tolerance(1.0, 1.0),
        "skin residual {} no longer exceeds the near-identity band",
        proof.skin_matrix.max()
    );
}

#[test]
fn a_synthetic_corner_from_three_vertices_does_not_shrink_the_bounds_tolerance() {
    // Each axis extreme comes from a *different* vertex, so the minimum
    // corner is `(0.001, 0.001, 0.002)` — magnitude `2.4e-3` — while
    // every vertex that contributed to it has magnitude `3000`. A
    // tolerance read off the corner is a million times tighter than the
    // rounding error the corner carries; one read off the magnitudes the
    // arithmetic ran on is not.
    let doc = rotating_rig_document(
        [
            Quat::from_xyzw(0.5992112, -0.6357324, 0.3481601, 0.33996314),
            Quat::from_xyzw(0.56926024, -0.14522065, -0.20381902, 0.7831421),
        ],
        3190.0,
        [Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.3, 0.4, 0.5)],
        &[
            Vec3::new(3000.0, 0.001, 0.002),
            Vec3::new(0.001, 3000.0, 0.003),
            Vec3::new(0.002, 0.003, 3000.0),
        ],
        &[[1.0, 0.0, 0.0, 0.0]; 3],
    );
    let plan = rest_bind_plan(&doc, 3190.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan)
        .expect("a synthetic corner must not shrink the tolerance");
    // The corner really is tiny relative to its own residual: without the
    // absolute term this comparison is `2.44e-4 <= 1.01e-6`.
    assert!(
        proof.bounds.max() > policy_scalar_tolerance_at(0.002),
        "bounds residual {} no longer exceeds the corner-derived band",
        proof.bounds.max()
    );
}

#[test]
fn a_joint_far_from_the_geometry_it_carries_still_proves_its_bounds() {
    // The magnitude of the *skinned points* is not on its own the
    // magnitude the bound's arithmetic ran on, and this is the fixture
    // that separates the two. The joints sit `3.2e6` units from the
    // origin (a `1000`-unit local translation under a `3190` root) while
    // every vertex is within one unit of its joint, so the skinned
    // extremes have magnitude `0.97` — but composing `W * B` cancelled
    // two `3.2e6`-magnitude terms, and the bound inherits that
    // cancellation's `1.56e-1` of error.
    //
    // Four ulps of the skinned magnitude alone is `4.6e-7`; four ulps of
    // the magnitude the composition ran on is `4.4`. The gap is a factor
    // of `9.6e6`, and only the second admits this correct candidate.
    let doc = far_joint_document();
    let plan = rest_bind_plan(&doc, 3190.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof =
        prove_scale(&doc, &candidate, &plan).expect("a distant joint's bounds must still prove");
    assert!(
        proof.bounds.max() > 4.0 * 1.0 * f64::from(f32::EPSILON),
        "bounds residual {} no longer exceeds four ulps of the skinned magnitude",
        proof.bounds.max()
    );
}

/// The far-joint rig: joints `3.2e6` from the origin carrying geometry
/// within one unit of themselves, so `W * B` is near-identity while the
/// composition that produced it ran on `6.4e6`.
///
/// This is the rig the calibration note and
/// [`ScaleTolerancePolicy::APPENDIX_D_V6`] quote the cost of the rounding
/// term on, so the two fixtures that read it share one definition. See
/// `docs/scale-calibration.md` for the recorded figure.
fn far_joint_document() -> Document {
    rotating_rig_document(
        [
            Quat::from_xyzw(0.84815156, -0.23002678, -0.2828825, -0.3843229),
            Quat::from_xyzw(0.6066518, -0.10115066, -0.7511764, 0.23974188),
        ],
        3190.0,
        [Vec3::new(0.0, 1000.0, 0.0), Vec3::new(200.0, -300.0, 400.0)],
        &[Vec3::new(0.5, -0.25, 0.125), Vec3::new(-0.75, 0.5, -0.25)],
        &[[1.0, 0.0, 0.0, 0.0], [0.5, 0.5, 0.0, 0.0]],
    )
}

/// One document side's composed skin slots, through the same helpers
/// [`check_skin_and_bounds`] composes them with.
///
/// The fixtures that separate the two documents' comparison bases have to
/// name both bases as numbers, and a base restated by hand in a fixture is
/// a base that stops describing the code the moment either moves.
fn rig_skin_slots(document: &Document) -> Vec<SkinSlot> {
    let worlds = rest_world_pose(&document.skeleton).expect("the rig composes");
    let instance = &document.assets.instances[0];
    instance
        .skin_joints
        .iter()
        .enumerate()
        .map(|(slot, &joint)| {
            SkinSlot::compose(
                worlds.bones[joint].matrix,
                instance_bind(document, instance, slot, joint).expect("the rig binds"),
                worlds.bones[joint].translation_rounding_magnitude,
            )
        })
        .collect()
}

/// The magnitude [`ProofResidualKind::SkinMatrix`] reads off one side.
fn rig_slot_magnitude(document: &Document) -> f64 {
    rig_skin_slots(document)
        .iter()
        .map(|slot| slot.rounding_magnitude)
        .fold(0.0, f64::max)
}

/// The magnitude [`ProofResidualKind::Bounds`] reads off one side.
fn rig_bounds_magnitude(document: &Document) -> f64 {
    let slots = rig_skin_slots(document);
    let instance = &document.assets.instances[0];
    let mut accumulator = BoundsAccumulator::default();
    for (primitive_index, primitive) in document.assets.meshes[instance.mesh]
        .primitives
        .iter()
        .enumerate()
    {
        accumulate_skinned_bounds(0, primitive_index, primitive, &slots, &mut accumulator)
            .expect("the rig skins");
    }
    accumulator.rounding_magnitude()
}

/// [`far_joint_document`] under whole-document conversion at `factor`,
/// which is the operation that puts the two sides' magnitudes a factor
/// apart: the joints stay `3.2e6` from the origin on the source side and
/// move to `3.2e6 * factor` on the candidate's, while the root's `3190`
/// linear part is untouched on both.
fn far_joint_conversion_at(factor: f64) -> (Document, ScalePlan, ScaleCandidate) {
    let doc = far_joint_document();
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        document: &doc,
        capability: &capability,
    })
    .expect("a whole-document conversion plans at any positive factor");
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    (doc, plan, candidate)
}

#[test]
fn the_far_joint_rig_admits_a_four_unit_bind_shift_and_refuses_the_next_one_up() {
    // The documented cost of the rounding term, pinned rather than
    // recomputed by hand each time it is quoted. `docs/scale-calibration.md`
    // records this floor, and this test holds it to the implementation — an
    // earlier revision stated `4.09`, which is on the *accepted* side of the
    // real floor and so described a bracket that does not exist.
    //
    // The floor is `4.09375`: a shift of exactly that much is admitted and
    // the next binary32 above it, `4.0937505`, is refused by `SkinMatrix`
    // at `observed: 3.0625` against `tolerance: 3.0423`. The observed
    // residual moves in steps rather than continuously, because it is the
    // stored bind's own `f32` quantization at `3.2e6` — which is what the
    // section says the term is covering.
    //
    // The refused shift is written as the successor of the accepted one
    // rather than as a decimal literal. This test's name claims a bracket
    // one representable step wide, and an earlier revision spent `4.094` —
    // some five hundred ulps up — which would have kept passing across any
    // widening of the band that stayed inside that gap.
    let doc = far_joint_document();
    let plan = rest_bind_plan(&doc, 3190.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let shifted = |shift: f32| {
        let mut broken = candidate.document().clone();
        broken.assets.instances[0].skin_ibms[0].w_axis.x += shift;
        prove_scale(&doc, &ScaleCandidate { document: broken }, &plan)
    };

    const FLOOR: f32 = 4.09375;
    let next_up = f32::from_bits(FLOOR.to_bits() + 1);
    shifted(FLOOR).expect("a 4.09375-unit bind shift is inside the documented floor");
    let error = shifted(next_up)
        .expect_err("the next binary32 above the floor, 4.0937505, must be refused");
    assert!(
        matches!(
            error,
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::SkinMatrix,
                ..
            }
        ),
        "expected a refused skin matrix just above the floor, got {error:?}"
    );
}

/// A half turn about `z`, exactly: the composed `W * B` of the second
/// slot in [`cancelling_blend_document`], written as literals because
/// `Quat::from_rotation_z(PI)` is not exact and the cancellation this
/// fixture depends on is.
const HALF_TURN_Z: Mat4 = Mat4::from_cols(
    Vec4::new(-1.0, 0.0, 0.0, 0.0),
    Vec4::new(0.0, -1.0, 0.0, 0.0),
    Vec4::new(0.0, 0.0, 1.0, 0.0),
    Vec4::new(0.0, 0.0, 0.0, 1.0),
);

/// Two skin slots whose composed `W * B` differ by [`HALF_TURN_Z`], and
/// one vertex `1000` units out weighted equally across both — so the
/// weighted sum cancels to the origin while each term still carries the
/// rounding of a `1000`-unit transform.
///
/// The joints themselves sit within `3.2e-3` of the origin, so neither
/// the composition magnitude nor the parent chain can stand in for the
/// vertex transform: every stage *except* that transform is small here,
/// which is what isolates the stage under test.
///
/// Unlike every other fixture in this section this one does **not** take
/// `skin_ibms` from [`rotating_rig_document`], whose binds are the
/// analytic inverse of the rest world and therefore make `W * B` the
/// identity on every slot. Identity slots cannot cancel against each
/// other at all, which is precisely why no fixture here caught the
/// cancellation: a bind pose that is not the rest pose is ordinary
/// content, and it is the only way to reach it. The binds are inverted in
/// `f64` for [`far_joint_overflow_document`]'s reason.
///
/// It composes to a *rotation*, so `abs(W * B)` is `1` — see
/// [`scaled_cancelling_blend_document`] for why that mattered.
fn cancelling_blend_document() -> Document {
    cancelling_blend_document_reaching(1000.0)
}

/// [`cancelling_blend_document`] with the vertex `reach` units out.
///
/// The reach is this rig's whole magnitude — every joint sits within
/// `3.2e-3` of the origin and the blend cancels to nothing — so it is what
/// the bounds comparison base reads, and a fixture that needs that base
/// large enough to hold two bands apart sets it here rather than building
/// a second rig.
fn cancelling_blend_document_reaching(reach: f32) -> Document {
    scaled_cancelling_blend_document(1.0, reach)
}

/// [`cancelling_blend_document_reaching`] with both composed slots carrying
/// a uniform scale of `scale`, so `abs(W * B)` is `scale` rather than `1`.
///
/// The blend still cancels — the two slots are `scale * I` and
/// `scale * HALF_TURN_Z`, which send `(reach, 0, 0)` to
/// `(scale * reach, 0, 0)` and its negation — but each term of the
/// cancelled sum now carries the rounding of a `scale * reach`-magnitude
/// transform rather than a `reach`-magnitude one. That product is the
/// magnitude the transform ran on, and `scale = 1` is the only value at
/// which reading `abs(p)` alone happens to name it.
fn scaled_cancelling_blend_document(scale: f32, reach: f32) -> Document {
    let composed = Mat4::from_scale(Vec3::splat(scale));
    composed_slot_document(
        CANCELLING_BLEND_ROTATIONS,
        3190.0,
        CANCELLING_BLEND_LOCALS,
        [composed, composed * HALF_TURN_Z],
        &[Vec3::new(reach, 0.0, 0.0)],
        &[[0.5, 0.5, 0.0, 0.0]],
    )
}

/// The invalid signed-weight rig from #336: two opposed slots and weights
/// `1.0` and `-0.99999`. Before negative weights were rejected, dividing
/// by their near-zero sum amplified the blend without bound and forced a
/// dedicated bounds-magnitude stage.
fn amplifying_blend_document() -> Document {
    composed_slot_document(
        CANCELLING_BLEND_ROTATIONS,
        3190.0,
        CANCELLING_BLEND_LOCALS,
        [Mat4::IDENTITY, HALF_TURN_Z],
        &[Vec3::new(1000.0, 0.0, 0.0)],
        &[[1.0, -0.99999, 0.0, 0.0]],
    )
}

/// [`amplifying_blend_document`]'s signed weights over two slots that
/// compose to the same transform. This was the worse pre-#336 case: both
/// numerator and denominator cancelled, so no finite ULP stage could
/// cover the accumulated rounding. It remains as a refusal fixture.
fn cancelling_numerator_blend_document() -> Document {
    composed_slot_document(
        CANCELLING_BLEND_ROTATIONS,
        3190.0,
        CANCELLING_BLEND_LOCALS,
        [Mat4::IDENTITY, Mat4::IDENTITY],
        &[Vec3::new(1000.0, 0.0, 0.0)],
        &[[1.0, -0.99999, 0.0, 0.0]],
    )
}

/// The reproducer's two rotations, reused by every cancelling-blend rig.
const CANCELLING_BLEND_ROTATIONS: [Quat; 2] = [
    Quat::from_xyzw(-0.81788284, 0.343121, -0.45392478, -0.085369624),
    Quat::from_xyzw(-0.12301501, 0.043325406, -0.015209139, 0.991342),
];

/// Joint locals small enough that every joint sits within `3.2e-3` of the
/// origin at root scale `3190`, so the composition magnitude and the parent
/// chain are both near-unit and cannot stand in for the vertex transform.
const CANCELLING_BLEND_LOCALS: [Vec3; 2] =
    [Vec3::new(0.0, 1e-6, 0.0), Vec3::new(0.3e-6, 0.4e-6, 0.5e-6)];

/// A two-joint rig at root scale `factor` whose composed `W_i * B_i` is
/// exactly `composed[i]`, by taking each inverse bind as
/// `W_i^-1 * composed[i]` in `f64`.
///
/// [`rotating_rig_document`] takes each bind as the analytic inverse of its
/// own rest world, so every slot composes to the identity and no two slots
/// can cancel a vertex between them. This builder is how a fixture states
/// what the slots compose *to*, which is the only way to reach either the
/// cancellation or a composed magnitude that is not `1`.
///
/// With [`CANCELLING_BLEND_LOCALS`] the joints sit within `3.2e-3` of the
/// origin, so the composition magnitude and the parent chain are both
/// near-unit and every stage of the bounds base except the vertex transform
/// is small by construction. The binds are inverted in `f64` for
/// [`far_joint_overflow_document`]'s reason.
fn composed_slot_document(
    rotations: [Quat; 2],
    factor: f32,
    locals: [Vec3; 2],
    composed: [Mat4; 2],
    points: &[Vec3],
    weights: &[[f32; 4]],
) -> Document {
    let mut doc = rotating_rig_document(rotations, factor, locals, points, weights);
    let first = Mat4::from_scale(Vec3::splat(factor))
        * Mat4::from_rotation_translation(rotations[0], locals[0]);
    let second = first * Mat4::from_rotation_translation(rotations[1], locals[1]);
    doc.assets.instances[0].skin_ibms = [first, second]
        .into_iter()
        .zip(composed)
        .map(|(world, composed)| (world.as_dmat4().inverse() * composed.as_dmat4()).as_mat4())
        .collect();
    doc
}

#[test]
fn two_slots_whose_composed_binds_cancel_a_vertex_still_prove_its_bounds() {
    // The blend can cancel just as the composition and the parent chain
    // can, and here it cancels completely:
    // `0.5 * (1000, 0, 0) + 0.5 * (-1000, 0, 0)` is the origin, so the
    // skinned point cannot itself reveal the transform's operands. The
    // joints are within `3.2e-3` of the origin, so the composition and
    // chain magnitudes are near `1`, while that transform still ran on
    // `1000`.
    //
    // Without the transform stage in the base this correct candidate is
    // refused at `observed: 6.1e-5` — one ulp of `1000` — against a
    // `1.48e-6` tolerance, a demand of `503` ulps. At `abs(p) = 1e5` the
    // same rig demands `65_527`, so this is not a count that could have
    // been raised.
    //
    // `abs(W * B)` is `1` on both slots here, because the two compose to a
    // rotation. That is what
    // `two_slots_with_a_scaled_composition_cancel_a_vertex_and_still_prove_its_bounds`
    // exists to stop being the only case reached: with it `1`, a base that
    // reads `abs(p)` alone is accidentally exact.
    let doc = cancelling_blend_document();
    let plan = rest_bind_plan(&doc, 3190.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // The cancellation is the fixture, so it is asserted before what it
    // costs: a rig that stopped cancelling passes below for want of a
    // defect rather than because the base covers the vertex.
    let worlds = rest_world_pose(&doc.skeleton).unwrap();
    let instance = &doc.assets.instances[0];
    let mut blended = Vec3::ZERO;
    let mut slot_magnitude = 0.0f64;
    for slot in 0..2 {
        let joint = instance.skin_joints[slot];
        let bind = instance_bind(&doc, instance, slot, joint).unwrap();
        let composed = SkinSlot::compose(
            worlds.bones[joint].matrix,
            bind,
            worlds.bones[joint].translation_rounding_magnitude,
        );
        blended += 0.5
            * composed
                .matrix
                .transform_point3(Vec3::new(1000.0, 0.0, 0.0));
        slot_magnitude = slot_magnitude.max(composed.rounding_magnitude);
    }
    assert!(
        blended.length() < 1.0 && slot_magnitude < 2.0,
        "the blend no longer cancels ({blended}) or the slots are no longer near-unit \
             ({slot_magnitude}): the vertex is then covered by another stage",
    );

    let proof = prove_scale(&doc, &candidate, &plan)
        .expect("a correct candidate whose slots cancel a vertex must still prove");
    assert!(
        proof.bounds.max() > 4.0 * slot_magnitude.max(1.0) * f64::from(f32::EPSILON),
        "bounds residual {} no longer exceeds four ulps of every stage but the vertex",
        proof.bounds.max()
    );
}

#[test]
fn two_slots_with_a_scaled_composition_cancel_a_vertex_and_still_prove_its_bounds() {
    // The stage above names the vertex, and the vertex alone is only half
    // of what its own transform runs on. `W * B * p` sums terms of size
    // `abs(W * B) * abs(p)`, and every rig in this module that reaches the
    // cancellation composes `W * B` from a *rotation* — `HALF_TURN_Z` — so
    // `abs(W * B)` is `1` and reading `abs(p)` alone was accidentally
    // exact. Nothing here could see the other factor.
    //
    // This rig supplies it: the two slots compose to `1024 * I` and
    // `1024 * HALF_TURN_Z`, so they still cancel a vertex to the origin,
    // and each term of the cancelled sum now carries the rounding of a
    // `1024 * 65536 = 6.7e7`-magnitude transform. Every earlier proxy is
    // still small — the blend is `0`, the joints are within `3.2e-3` of
    // the origin, and the vertex itself is `65536`.
    //
    // With `abs(p)` alone in the base this correct candidate is refused at
    // `observed: 4.0` against `tolerance: 0.0313`, a demand of `512` ulps.
    // The gap is `min(abs(W * B), abs(p))` and grows without bound with it:
    // the same rig refuses from `min(scale, reach) > 8` upward.
    let doc = scaled_cancelling_blend_document(1024.0, 65536.0);
    let plan = rest_bind_plan(&doc, 3190.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // The rig's shape is the fixture, so it is asserted before what it
    // costs: a composition that stopped scaling, or a blend that stopped
    // cancelling, passes below for want of a defect rather than because
    // the base covers the transform.
    let slots = rig_skin_slots(&doc);
    let position = doc.assets.meshes[0].primitives[0].positions[0];
    let blended: Vec3 = slots
        .iter()
        .map(|slot| 0.5 * slot.matrix.transform_point3(position))
        .sum();
    let composed_scale = slots[0].matrix.x_axis.truncate().length();
    let stages_without_the_transform = slots
        .iter()
        .map(|slot| slot.rounding_magnitude)
        .fold(0.0, f64::max)
        .max(f64::from(position.length()))
        .max(f64::from(blended.length()));
    assert!(
        composed_scale > 512.0 && blended.length() < 0.01 * position.length(),
        "the composition no longer scales ({composed_scale}) or the blend no longer cancels \
             ({blended}): the transform's operand product is then covered by another stage",
    );
    assert!(
        stages_without_the_transform < 1e5,
        "some stage other than the transform now reaches {stages_without_the_transform}, so \
             this fixture no longer isolates the transform's operand product",
    );

    let proof = prove_scale(&doc, &candidate, &plan).expect(
        "a correct candidate whose scaled slots cancel a vertex must still prove: the \
             transform ran on abs(W * B) * abs(p), and that is what the base must name",
    );
    assert!(
        proof.bounds.max() > 4.0 * stages_without_the_transform * f64::from(f32::EPSILON),
        "bounds residual {} no longer exceeds four ulps of every stage but the transform's \
             own operand product",
        proof.bounds.max()
    );
}

#[test]
fn both_signed_blend_counterexamples_are_rejected_for_both_operations() {
    let capability = complete_capability();
    let operations = [
        ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 3190.0,
        },
        ScaleOperation::WholeDocumentLinearUnits { factor: 3190.0 },
    ];
    for doc in [
        amplifying_blend_document(),
        cancelling_numerator_blend_document(),
    ] {
        for operation in operations {
            assert_eq!(
                plan_scale(&ScaleRequest {
                    operation,
                    document: &doc,
                    capability: &capability,
                })
                .unwrap_err(),
                ScaleError::NegativeSkinWeight {
                    mesh_index: 0,
                    primitive_index: 0,
                    vertex_index: 0,
                    influence_index: 1,
                }
            );
        }
    }
}

#[test]
fn a_growing_conversion_reads_the_candidate_s_magnitude_not_the_source_s_unrebased() {
    // Both obligations take `candidate.max(q * source)`, and every
    // rest/bind fixture above is blind to where that magnitude comes from:
    // rest/bind moves the factor from the root's scale into the joint
    // translations and leaves `q = 1`, so the source composes `3190 * 1000`
    // where the candidate composes `1 * 3.19e6` and the two sides coincide
    // by construction.
    //
    // Whole-document conversion separates them. Here the source's
    // composition magnitude is `9.30e6` and the candidate's `2.97e10`,
    // `3190x` apart, and both residuals sit far above what the source side
    // alone would buy: `798` and `1595` against a source-derived band of
    // `4.44`. Reading the source magnitude unrebased refuses this correct
    // candidate by three orders of magnitude.
    //
    // What this fixture cannot do is separate `candidate` from
    // `q * source`. At a growing factor the two sides are exactly the
    // factor apart, so `q * source` and `candidate` are the same number to
    // the digit — which is why an earlier revision of this test claimed
    // that dropping *either* side of a `max(source, candidate)` refused the
    // correct candidate, and why that claim was false: flipping both
    // obligations to the candidate side alone left every test in this
    // module passing.
    // `a_shrinking_conversion_rebases_the_skin_matrix_magnitude_by_the_factor`
    // and `a_shrinking_conversion_rebases_the_bounds_magnitude_by_the_factor`
    // are the direction where the two do come apart, and they are what
    // holds the rebasing in place.
    let (doc, plan, candidate) = far_joint_conversion_at(3190.0);
    let source_slot = rig_slot_magnitude(&doc);
    let candidate_slot = rig_slot_magnitude(candidate.document());
    assert!(
        candidate_slot > 1000.0 * source_slot,
        "the candidate is no longer the larger side, so this fixture no longer refuses \
             the source-side reading: {candidate_slot} / {source_slot}",
    );

    let proof = prove_scale(&doc, &candidate, &plan).expect(
        "the two documents' composition magnitudes are 3190x apart, and the candidate's \
             arithmetic is what both obligations must be given room for",
    );
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    let source_band = policy.f32_rounded_tolerance(0.0, 0.0, source_slot);
    assert!(
        proof.skin_matrix.max() > source_band,
        "skin matrix residual {} no longer exceeds the {source_band} the source side buys, \
             so this fixture no longer kills the unrebased source reading",
        proof.skin_matrix.max(),
    );
    assert!(
        proof.bounds.max() > policy.f32_rounded_tolerance(0.0, 0.0, rig_bounds_magnitude(&doc)),
        "bounds residual {} no longer exceeds what the source side buys",
        proof.bounds.max(),
    );
}

#[test]
fn a_shrinking_conversion_rebases_the_skin_matrix_magnitude_by_the_factor() {
    // The direction that separates the candidate's magnitude from the
    // source's, and the one the `max` over the two sides was loose in.
    //
    // The far-joint rig composes `W * B` on `9.30e6` at every factor —
    // the joints are `3.2e6` out and the root's `3190` linear part is
    // untouched by conversion — while the candidate composes it on
    // `9.30e6 * factor`. Under a shrinking conversion the source is
    // therefore the *larger* side, and a `max` hands this obligation a
    // band derived from a rig `1/factor` times bigger than the one the
    // residual was measured on. It is not merely redundant there, it is
    // loose by exactly `1/factor`: the band freezes at `4.44` while the
    // candidate it is spent on keeps shrinking.
    //
    // Measured: the smallest inverse-bind shift this obligation refuses is
    // `1.3e-5` at `0.01` and `1.3e-7` at `1e-4`, against `1.9e-3` under the
    // `max` at both — `100x` and `10_000x` of recovered discriminating
    // power, tracking the factor as it should.
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    for (factor, shift) in [(0.01f64, 1e-4f32), (1e-4, 1e-6)] {
        let (doc, plan, candidate) = far_joint_conversion_at(factor);
        let source_slot = rig_slot_magnitude(&doc);
        let candidate_slot = rig_slot_magnitude(candidate.document());
        assert!(
            source_slot > 50.0 * candidate_slot,
            "the source side is no longer the larger one at {factor}, so this fixture no \
                 longer separates the rebased magnitude from the max: {source_slot} / \
                 {candidate_slot}",
        );

        // The equivalence half: a correct candidate still proves against
        // the rebased base, at the end of the range where it is the
        // *smaller* of the two.
        prove_scale(&doc, &candidate, &plan)
            .expect("a correct candidate under a shrinking conversion must still prove");

        // And the tightening half. The shift sits between the two bands:
        // above what the candidate's own composition buys, below the
        // `4.44` the source's would.
        let mut broken = candidate.document().clone();
        broken.assets.instances[0].skin_ibms[0].w_axis.x += shift;
        let broken = ScaleCandidate { document: broken };
        let error = prove_scale(&doc, &broken, &plan)
            .expect_err("a bind shift above the rebased band must be refused");
        let ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::SkinMatrix,
            observed,
            tolerance,
        } = error
        else {
            panic!("expected a refused skin matrix at {factor}, got {error:?}");
        };
        assert!(
            observed > tolerance,
            "skin matrix band moved at {factor}: observed {observed}, tolerance {tolerance}"
        );
        assert!(
            observed < policy.f32_rounded_tolerance(0.0, 0.0, source_slot),
            "skin matrix residual {observed} now exceeds what the unrebased source \
                 magnitude buys too, so this fixture no longer kills the max at {factor}",
        );
    }
}

#[test]
fn a_shrinking_conversion_rebases_the_bounds_magnitude_by_the_factor() {
    // The same direction for the bounds obligation, on the rig whose
    // magnitude *is* its geometry: two slots a half turn apart cancel a
    // `1e6`-unit vertex to the origin, so the source's bounds base is
    // `1e6` and the candidate's is `1e6 * factor` while every joint stays
    // within `3.2e-3` of the origin.
    //
    // The defect is a weight, not a position: whole-document conversion
    // rewrites mesh positions, so a moved vertex is refused by
    // `MeshPosition` before the bounds comparison is ever reached. A
    // reweighted vertex is a build that blended the same two slots
    // differently, which the bounds obligation is the only one that sees.
    // `0.5 +/- 1e-6` unbalances the cancellation by `2e-6` of the reach.
    //
    // Measured: the smallest bounds error refused is `4.77e-3` at `0.01`
    // and `4.87e-5` at `1e-4`, against `4.77e-1` under the `max` at both.
    // That is `100x` and `9797x` of recovered discriminating power — the
    // second short of `10_000x` only because the `1e-6` absolute band
    // starts to pay at that size.
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    for &factor in &[0.01f64, 1e-4] {
        let doc = cancelling_blend_document_reaching(1e6);
        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor },
            document: &doc,
            capability: &capability,
        })
        .expect("a whole-document conversion plans at any positive factor");
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let source_bounds = rig_bounds_magnitude(&doc);
        let candidate_bounds = rig_bounds_magnitude(candidate.document());
        assert!(
            source_bounds > 50.0 * candidate_bounds,
            "the source side is no longer the larger one at {factor}: {source_bounds} / \
                 {candidate_bounds}",
        );

        prove_scale(&doc, &candidate, &plan)
            .expect("a correct candidate under a shrinking conversion must still prove");

        let mut broken = candidate.document().clone();
        broken.assets.meshes[0].primitives[0].weights[0] = [0.5 + 1e-6, 0.5 - 1e-6, 0.0, 0.0];
        let broken = ScaleCandidate { document: broken };
        let error = prove_scale(&doc, &broken, &plan)
            .expect_err("a reweighted blend above the rebased band must be refused");
        let ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::Bounds,
            observed,
            tolerance,
        } = error
        else {
            panic!("expected a refused bound at {factor}, got {error:?}");
        };
        assert!(
            observed > tolerance,
            "bounds band moved at {factor}: observed {observed}, tolerance {tolerance}"
        );
        assert!(
            observed < policy.f32_rounded_tolerance(0.0, 0.0, source_bounds),
            "bounds residual {observed} now exceeds what the unrebased source magnitude \
                 buys too, so this fixture no longer kills the max at {factor}",
        );
    }
}

#[test]
fn a_growing_conversion_provisions_a_rebased_source_magnitude() {
    // `candidate.max(q * source)` does not collapse to `candidate` as an
    // expression: under a growing conversion `q * source` can exceed the
    // candidate's own magnitude, because the two sides are a factor apart
    // only in the terms that carry a translation. Both retain an unscaled
    // `O(1)` floor — the composition's linear block, and the exact `1.0`
    // the homogeneous row contributes to every parent chain — and where
    // that floor is what dominates the source, `q * source` runs away from
    // `candidate` by up to the whole factor.
    //
    // This rig is that regime: joints within `3e-4` of the origin carrying
    // geometry within `4e-4` of themselves, so both sides read `1.00` and
    // `1.80` while `3190 * source` is `3190` — a `1776x` excess.
    //
    // The proof must retain this analytically rebased source rounding
    // provision even when the candidate term is much smaller.
    let doc = rotating_rig_document(
        [
            Quat::from_xyzw(-0.4142528, 0.36644182, -0.4827506, -0.67901903),
            Quat::from_xyzw(0.4266807, 0.07081859, 0.56996834, -0.698616),
        ],
        1.0,
        [
            Vec3::new(-1.0505125e-7, 2.1474386e-6, 1.2482113e-6),
            Vec3::new(-2.8671836e-4, -6.197663e-5, -3.234957e-5),
        ],
        &[
            Vec3::new(6.400283e-6, -6.33053e-6, -1.0025909e-6),
            Vec3::new(9.629462e-5, -4.045991e-5, 4.1240072e-4),
        ],
        &[[0.5, 0.5, 0.0, 0.0]; 2],
    );
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 3190.0 },
        document: &doc,
        capability: &capability,
    })
    .expect("a whole-document conversion plans at any positive factor");
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    let source_slot = rig_slot_magnitude(&doc);
    let candidate_slot = rig_slot_magnitude(candidate.document());
    let source_bounds = rig_bounds_magnitude(&doc);
    let candidate_bounds = rig_bounds_magnitude(candidate.document());
    assert!(
        3190.0 * source_slot > 100.0 * candidate_slot
            && 3190.0 * source_bounds > 100.0 * candidate_bounds,
        "the rebased source magnitude no longer runs away from the candidate's, so this \
             rig no longer reaches the regime: slots {source_slot} / {candidate_slot}, bounds \
             {source_bounds} / {candidate_bounds}",
    );

    let proof = prove_scale(&doc, &candidate, &plan)
        .expect("a correct candidate under a growing conversion must prove");
    assert!(
        proof.skin_matrix.comparisons() > 0 && proof.bounds.comparisons() > 0,
        "the growing-conversion fixture must evaluate both provisioned obligations",
    );
}

#[test]
fn a_rig_whose_skinned_extent_passes_the_square_root_of_f32_max_still_proves() {
    // Bounds comparisons are per axis, so deriving their provenance from
    // the blended point's L2 length would both use the wrong norm and
    // square in `f32`. That would put a hard refusal boundary at
    // `sqrt(f32::MAX) = 1.845e19`, nineteen decades below the `3.403e38`
    // the rest of this proof stays finite to. Past that boundary the
    // magnitude becomes `inf`, and `check_residual` refuses a *correct*
    // candidate whose residual is exactly `0.0`.
    //
    // `1.9e19` is just past it, so this fixture fails in the most
    // misleading possible way if the length is ever narrowed back to
    // `f32`. Nothing else about the rig is unusual: it is the far-joint
    // fixture's rotations with one large coordinate.
    let doc = rotating_rig_document(
        [
            Quat::from_xyzw(0.84815156, -0.23002678, -0.2828825, -0.3843229),
            Quat::from_xyzw(0.6066518, -0.10115066, -0.7511764, 0.23974188),
        ],
        1e3,
        [Vec3::new(0.0, 1000.0, 0.0), Vec3::new(200.0, -300.0, 400.0)],
        &[Vec3::new(1.9e19, 0.0, 0.0), Vec3::new(-0.75, 0.5, -0.25)],
        &[[1.0, 0.0, 0.0, 0.0], [0.5, 0.5, 0.0, 0.0]],
    );
    let plan = rest_bind_plan(&doc, 1e3);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).expect(
        "a rig whose skinned extent exceeds sqrt(f32::MAX) must still prove: the extent \
             itself is finite, and squaring it is the proof's own arithmetic",
    );
    assert!(
        proof.bounds.max().is_finite(),
        "bounds residual {} is not finite",
        proof.bounds.max()
    );
}

/// The far-joint rotations, a `1e35` joint offset, and inverse binds
/// that really are the inverses of the rest world matrices.
///
/// The offset puts the joint's world translation at `3.19e38` — inside
/// `f32`, an eighteenth of the way below `f32::MAX` — which is what makes
/// `abs(W) * abs(B)` overflow while `W * B` itself stays near-identity.
/// The inverse binds are inverted in `f64` because `glam`'s `f32`
/// `Mat4::inverse` expands cofactors, and at this factor its own
/// intermediates overflow: it would refuse the fixture at
/// `non_finite_inverse_bind` before the magnitude was ever computed. A
/// stored inverse bind comes off disk as sixteen numbers, not from a
/// 32-bit cofactor expansion, so inverting exactly is what a real file
/// carries here.
fn far_joint_overflow_document() -> (Document, Mat4, Mat4) {
    let rotations = [
        Quat::from_xyzw(0.84815156, -0.23002678, -0.2828825, -0.3843229),
        Quat::from_xyzw(0.6066518, -0.10115066, -0.7511764, 0.23974188),
    ];
    let locals = [Vec3::new(0.0, 1e35, 0.0), Vec3::new(0.3, 0.4, 0.5)];
    let mut doc = rotating_rig_document(
        rotations,
        3190.0,
        locals,
        &[Vec3::new(1.0, 0.0, 0.0), Vec3::new(-0.75, 0.5, -0.25)],
        &[[1.0, 0.0, 0.0, 0.0], [0.5, 0.5, 0.0, 0.0]],
    );
    let first = Mat4::from_scale(Vec3::splat(3190.0))
        * Mat4::from_rotation_translation(rotations[0], locals[0]);
    let second = first * Mat4::from_rotation_translation(rotations[1], locals[1]);
    let inverse_binds = vec![
        first.as_dmat4().inverse().as_mat4(),
        second.as_dmat4().inverse().as_mat4(),
    ];
    doc.assets.instances[0].skin_ibms.clone_from(&inverse_binds);
    (doc, first, inverse_binds[0])
}

#[test]
fn a_rig_whose_composition_operands_overflow_f32_still_proves() {
    // `product_operand_magnitude` sums products of two `f32` operand
    // entries, so it runs past `f32::MAX` on operands that are themselves
    // finite — and it does so precisely where the cancellation that makes
    // `W * B` near-identity has taken the magnitude *out* of the result,
    // which is the case the magnitude exists to describe. Sweeping
    // 2_000_000 random rig-shaped `W` / `B` pairs found 87 of them.
    //
    // Computed in `f32` lanes the magnitude here is `inf`, which makes
    // every tolerance derived from it `inf`, which `check_residual`
    // refuses — a correct candidate rejected with `tolerance: inf`. This
    // rig reaches it from the joint transforms alone: the mesh it carries
    // is two unit-scale points.
    let (doc, world, inverse_bind) = far_joint_overflow_document();

    assert!(
        !largest_entry(mat4_abs(world) * mat4_abs(inverse_bind)).is_finite(),
        "the fixture no longer overflows the f32 lane computation, so it no longer \
             exercises the fallback",
    );
    assert!(
        (world * inverse_bind).is_finite(),
        "the composition itself must stay finite: an overflowing product is a different \
             failure, and one that is allowed to be refused",
    );
    let slot = SkinSlot::compose(world, inverse_bind, 0.0);
    assert!(
        slot.rounding_magnitude.is_finite(),
        "rounding magnitude {} is not finite",
        slot.rounding_magnitude
    );

    let plan = rest_bind_plan(&doc, 3190.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).expect(
        "a correct candidate whose composition operands overflow f32 must still prove: \
             both operands are finite and so is their product",
    );
    assert!(
        proof.skin_matrix.max().is_finite() && proof.bounds.max().is_finite(),
        "skin {} / bounds {} residual is not finite",
        proof.skin_matrix.max(),
        proof.bounds.max()
    );
}

/// The far-joint rotations, a root factor of `1e3`, and a second joint
/// whose local translation is `-R0^-1 t0` — so the chain's own
/// `abs(W) * abs(t_local)` sums past `f32::MAX` while the world
/// translation they produce cancels to near zero.
///
/// `3e35` under a `1e3` root puts the first joint's world translation at
/// `3e38`, and the second joint's operand sum at
/// `1e3 * 3e35 + 3e38 = 6e38`. The inverse binds are inverted in `f64`
/// for [`far_joint_overflow_document`]'s reason.
fn cancelling_chain_overflow_document() -> Document {
    let rotations = [
        Quat::from_xyzw(0.84815156, -0.23002678, -0.2828825, -0.3843229),
        Quat::from_xyzw(0.6066518, -0.10115066, -0.7511764, 0.23974188),
    ];
    // The second local is `-R0^-1 (0, 3e35, 0)`, as literals.
    let locals = [
        Vec3::new(0.0, 3e35, 0.0),
        Vec3::new(5.1827603e34, 1.7963013e35, -2.3462077e35),
    ];
    let mut doc = rotating_rig_document(
        rotations,
        1e3,
        locals,
        &[Vec3::new(1.0, 0.0, 0.0), Vec3::new(-0.75, 0.5, -0.25)],
        &[[1.0, 0.0, 0.0, 0.0], [0.5, 0.5, 0.0, 0.0]],
    );
    let first = Mat4::from_scale(Vec3::splat(1e3))
        * Mat4::from_rotation_translation(rotations[0], locals[0]);
    let second = first * Mat4::from_rotation_translation(rotations[1], locals[1]);
    doc.assets.instances[0].skin_ibms = vec![
        first.as_dmat4().inverse().as_mat4(),
        second.as_dmat4().inverse().as_mat4(),
    ];
    doc
}

/// The reproducer rotations and a second joint whose local translation is
/// `-R0^-1 t0`, so the two terms its world translation is composed from
/// cancel: at `factor` the chain runs on `factor * 1000` per term and the
/// surviving world translation is a rounding artefact of it.
///
/// The cancellation is a property of the two locals alone, not of
/// `factor` — the root scales both terms equally — so the same rig serves
/// rest/bind at `3190` and whole-document conversion from `1`.
fn cancelling_chain_document(factor: f32) -> Document {
    rotating_rig_document(
        [
            Quat::from_xyzw(-0.81788284, 0.343121, -0.45392478, -0.085369624),
            Quat::from_xyzw(-0.12301501, 0.043325406, -0.015209139, 0.991342),
        ],
        factor,
        // The second local is `-R0^-1 (0, 1000, 0)`, as literals.
        [
            Vec3::new(0.0, 1000.0, 0.0),
            Vec3::new(483.7628, 749.96, 451.14697),
        ],
        &[Vec3::new(0.5, -0.25, 0.125), Vec3::new(-0.75, 0.5, -0.25)],
        &[[1.0, 0.0, 0.0, 0.0], [0.5, 0.5, 0.0, 0.0]],
    )
}

#[test]
fn a_parent_chain_whose_translations_cancel_still_proves_its_skin() {
    // The magnitude a *composition* rounds against is not on its own the
    // magnitude its `world` operand arrived carrying, and this is the
    // fixture that separates the two. The second joint's local
    // translation is `-R0^-1 t0`, so its world translation is the
    // difference of two terms of magnitude `3190 * 1000 = 3.19e6` and
    // lands at `(0.125, 0, -0.125)`.
    //
    // `product_operand_magnitude(W, B)` reads `1.0` there, and correctly:
    // that really is what `W * B` rounds against, because `W`'s
    // translation column no longer contains the terms that cancelled. The
    // composed skin matrix nonetheless differs between the two documents
    // by `6.25e-2`, which is `524288` binary32 ulps of that base and
    // `0.082` of the parent chain's.
    let doc = cancelling_chain_document(3190.0);
    let plan = rest_bind_plan(&doc, 3190.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // The cancellation is the fixture, so it is asserted before what it
    // costs: a rig that stopped cancelling fails here rather than passing
    // for want of a defect.
    let worlds = rest_world_pose(&doc.skeleton).unwrap();
    let instance = &doc.assets.instances[0];
    let joint = instance.skin_joints[1];
    let bind = instance_bind(&doc, instance, 1, joint).unwrap();
    let product = product_operand_magnitude(worlds.bones[joint].matrix, bind);
    assert!(
        product < 2.0,
        "the composed product's own operands are no longer near-unit: {product}"
    );
    assert!(
        worlds.bones[joint].translation_rounding_magnitude > 1e6,
        "the parent chain no longer cancels a large translation: {}",
        worlds.bones[joint].translation_rounding_magnitude
    );

    let proof = prove_scale(&doc, &candidate, &plan)
        .expect("a correct candidate under a cancelling parent chain must prove");
    assert!(
        proof.skin_matrix.max() > 4.0 * product * f64::from(f32::EPSILON),
        "skin residual {} no longer exceeds four ulps of the composition's own operands",
        proof.skin_matrix.max()
    );
    // The same cancellation is visible one obligation earlier: the world
    // translation it produced is `0.177` long and carries `3.19e6`'s
    // rounding, so a purely per-axis band on the surviving translation
    // refuses this node before the skin matrix is ever composed. Asserted
    // here rather than left implicit, so `RestTranslation` losing the term
    // fails a claim this fixture makes rather than one it happens to
    // reach first.
    let world_translation = worlds.bones[joint].matrix.w_axis.truncate().length() as f64;
    assert!(
        proof.rest_translation.max()
            > ScaleTolerancePolicy::APPENDIX_D_V6
                .scalar_tolerance(world_translation, world_translation),
        "rest translation residual {} no longer exceeds the per-axis band its own \
             translation buys",
        proof.rest_translation.max()
    );
}

/// [`cancelling_chain_document`] under whole-document conversion at
/// `factor`, which is the only operation that puts the two documents'
/// chain magnitudes apart: rest/bind moves the factor from the root's
/// scale into the joint translations, so both sides compose the same
/// `3.19e6` and every rest/bind fixture is blind to which side the
/// magnitude is read from.
///
/// The source chain is `2000` at every factor and the candidate's is
/// `2000 * factor`, so the growing and shrinking directions are the same
/// rig with the sign of `log(factor)` flipped.
fn cancelling_chain_conversion_at(factor: f64) -> (Document, ScalePlan, ScaleCandidate) {
    let doc = cancelling_chain_document(1.0);
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        document: &doc,
        capability: &capability,
    })
    .expect("a whole-document conversion plans at any positive factor");
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    (doc, plan, candidate)
}

/// [`cancelling_chain_conversion_at`] at `3190`, where the candidate's
/// chain is `6.38e6` against the source's `2000` and the residual the
/// candidate's rounding leaves is `0.197`.
fn cancelling_chain_conversion() -> (Document, ScalePlan, ScaleCandidate) {
    cancelling_chain_conversion_at(3190.0)
}

#[test]
fn a_cancelling_chain_under_conversion_holds_rest_translation_to_the_candidate_side() {
    // `RestTranslation`'s magnitude is the *candidate's* parent chain, and
    // this is the fixture that pins it is a chain at all and that it is
    // read from the candidate. The chain cancels, so the surviving world
    // translation is `6.1e-5` on the source side and the residual between
    // the two documents is `0.197` — nothing the compared translations'
    // own magnitudes could ever buy. And the two chains are `3190x` apart,
    // so reading the source side instead buys the candidate's arithmetic a
    // band derived from a rig `3190` times smaller.
    //
    // Measured: the residual is `0.197` against `3.04` from the candidate's
    // chain, `9.58e-4` from the source's, and `3.92e-6` with no chain term
    // at all. Only the first admits this correct candidate.
    //
    // This is the growing direction, where the candidate's chain is also
    // the larger of the two;
    // `a_shrinking_conversion_holds_rest_translation_to_the_candidate_s_own_chain`
    // is the direction that separates "the candidate's" from "the larger".
    let (doc, plan, candidate) = cancelling_chain_conversion();
    let source_chain =
        rest_world_pose(&doc.skeleton).unwrap().bones[2].translation_rounding_magnitude;
    let candidate_chain = rest_world_pose(&candidate.document().skeleton)
        .unwrap()
        .bones[2]
        .translation_rounding_magnitude;
    assert!(
        candidate_chain > 1e3 * source_chain,
        "the two sides' chains are no longer a factor apart: {source_chain} / \
             {candidate_chain}",
    );

    let proof = prove_scale(&doc, &candidate, &plan).expect(
        "the two documents' chain magnitudes are 3190x apart, and the candidate's \
             arithmetic is what this obligation must be given room for",
    );
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    assert!(
        proof.rest_translation.max() > policy.f32_rounded_tolerance(0.0, 0.0, source_chain),
        "rest translation residual {} no longer exceeds what the source side's chain \
             buys, so this fixture no longer separates the two sides",
        proof.rest_translation.max()
    );
}

#[test]
fn a_shrinking_conversion_holds_rest_translation_to_the_candidate_s_own_chain() {
    // The other end of the factor range, and the fixture the equivalence
    // argument for `max(before_chain, after_chain)` used to stand in for.
    //
    // At `0.01` the *source* chain is the larger of the two — `2000`
    // against the candidate's `20` — so a `max` over the two sides hands
    // this obligation a band derived from a rig `100x` bigger than the one
    // the residual was measured on. It is not merely redundant there, it
    // is loose by `1/factor`: the band freezes at the source rig's size
    // while the candidate the band is spent on keeps shrinking.
    //
    // Measured: with the `max` the smallest refused displacement of the
    // affected joint is `9.54e-4` at `0.01` and `9.55e-4` at `1e-4` —
    // pinned to the source rig at both. With the candidate's chain alone
    // it is `1.07e-5` and `1.47e-6`: `89x` and `648x` tighter, and it
    // tracks the factor as it should.
    //
    // The defect goes on `unskinned_sibling_document`'s unskinned bone,
    // not on a skin joint. An earlier revision put it on joint `2` and
    // read as killing both mutations; it did not. Displacing a skin joint
    // moves `W * B` too, `SkinMatrix` refuses at `8.0e-5` against its own
    // band before this obligation's band decides anything, and the
    // `expect_err` below then panics on the wrong kind — a coincidental
    // death, not a detection.
    let (doc, plan, candidate) = unskinned_sibling_conversion(false, 0.01);
    let source_chain = rest_world_pose(&doc.skeleton).unwrap().bones[UNSKINNED_SIBLING_BONE]
        .translation_rounding_magnitude;
    let candidate_chain = rest_world_pose(&candidate.document().skeleton)
        .unwrap()
        .bones[UNSKINNED_SIBLING_BONE]
        .translation_rounding_magnitude;
    assert!(
        source_chain > 50.0 * candidate_chain,
        "the source side is no longer the larger one, so this fixture no longer \
             separates the candidate's chain from the max: {source_chain} / {candidate_chain}",
    );

    // The equivalence half: a correct candidate still proves on the
    // candidate's chain alone, at the end of the range where that chain is
    // the *smaller* of the two. Asserted together with the isolation, on a
    // `1e-5` displacement of the same axis that both bands admit.
    let mut nudged = candidate.document().clone();
    nudged.skeleton.bones[UNSKINNED_SIBLING_BONE]
        .rest
        .translation
        .x += 1e-5;
    assert_the_defect_axis_is_invisible_to_the_skin(
        &doc,
        &plan,
        &candidate,
        &ScaleCandidate { document: nudged },
        |document: &Document| {
            document.skeleton.bones[UNSKINNED_SIBLING_BONE]
                .rest
                .translation
                .x
        },
    );

    // And the tightening half. `1e-4` sits between the two bands: above
    // the `1.05e-5` the candidate's chain buys, below the `9.54e-4` the
    // source's would. Reading the source side — or the `max`, which is the
    // source side here — admits this wrong candidate.
    let mut broken = candidate.document().clone();
    broken.skeleton.bones[UNSKINNED_SIBLING_BONE]
        .rest
        .translation
        .x += 1e-4;
    let broken = ScaleCandidate { document: broken };
    let error = prove_scale(&doc, &broken, &plan)
        .expect_err("a 1e-4 joint displacement must be refused on the candidate's chain");
    let ScaleError::ProofResidualExceeded {
        kind: ProofResidualKind::RestTranslation,
        observed,
        tolerance,
    } = error
    else {
        panic!("expected a refused rest translation, got {error:?}");
    };
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    assert!(
        observed > tolerance,
        "rest translation band moved: observed {observed}, tolerance {tolerance}"
    );
    assert!(
        observed < policy.f32_rounded_tolerance(0.0, 0.0, source_chain),
        "rest translation residual {observed} now exceeds what the source chain buys too, \
             so this fixture no longer kills the source-side reading",
    );
}

#[test]
fn the_rest_translation_v6_floor_is_an_adjacent_f32_transition() {
    // The over-acceptance direction for `RestTranslation`, which the
    // acceptance fixtures above cannot reach: a term that is merely
    // *present* is not yet a term of the right size. Search the complete
    // positive-binary32 interval from zero through a known refusal, sample
    // it for gross non-monotonicity, and pin the adjacent transition found
    // by bisection. This is a fixture-local floor, not a claim about every
    // possible defect.
    //
    // On `unskinned_sibling_document`'s unskinned bone, for the reason
    // that rig exists: on a skin joint, `SkinMatrix` refuses a `10`-unit
    // displacement at `7.97` against its own band and this bracket never
    // reaches the chain term at all.
    let (doc, plan, candidate) = unskinned_sibling_conversion(false, 3190.0);
    let refuses = |delta: f32| {
        let mut document = candidate.document().clone();
        document.skeleton.bones[UNSKINNED_SIBLING_BONE]
            .rest
            .translation
            .x += delta;
        match prove_scale(&doc, &ScaleCandidate { document }, &plan) {
            Ok(_) => false,
            Err(ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::RestTranslation,
                ..
            }) => true,
            Err(error) => panic!("the floor stopped belonging to RestTranslation: {error:?}"),
        }
    };
    let (accepted, refused) = adjacent_positive_refusal(10.0, refuses);
    assert_eq!(
        (accepted.to_bits(), refused.to_bits()),
        (0x4092_0000, 0x4092_0001),
    );
    assert!(refused.to_bits() > V4_REST_TRAJECTORY_REFUSED_TRANSITION_BITS);

    // Pin the accepted endpoint's isolation too: its skinned residuals
    // must be bit-identical to the clean candidate's, otherwise an earlier
    // obligation could be what owns the apparent transition.
    let mut inside = candidate.document().clone();
    inside.skeleton.bones[UNSKINNED_SIBLING_BONE]
        .rest
        .translation
        .x += accepted;
    assert_the_defect_axis_is_invisible_to_the_skin(
        &doc,
        &plan,
        &candidate,
        &ScaleCandidate { document: inside },
        |document: &Document| {
            document.skeleton.bones[UNSKINNED_SIBLING_BONE]
                .rest
                .translation
                .x
        },
    );
    let mut first_refused = candidate.document().clone();
    first_refused.skeleton.bones[UNSKINNED_SIBLING_BONE]
        .rest
        .translation
        .x += refused;
    let (observed, tolerance) = residual_refusal(
        prove_scale(
            &doc,
            &ScaleCandidate {
                document: first_refused,
            },
            &plan,
        )
        .unwrap_err(),
        ProofResidualKind::RestTranslation,
    );
    assert_eq!(
        (observed.to_bits(), tolerance.to_bits()),
        (0x4012_464d_554e_2270, 0x4012_40e6_ce11_6746),
        "the exact RestTranslation floor measurement drifted",
    );
}

/// The bone [`unskinned_sibling_document`] adds, and the whole point of
/// that rig: it is inside the affected closure, so `RestTranslation` and
/// `Trajectory` compare it, and it is in no instance's `skin_joints`, so
/// `check_skin_and_bounds` never reaches it.
const UNSKINNED_SIBLING_BONE: BoneId = 3;

/// [`cancelling_chain_document`] at root scale `1` plus a fourth bone that
/// **no vertex is weighted to**, carrying the same local as the skinned
/// second joint and hanging off the same parent — so its parent chain
/// cancels identically and its world translation lands within `0.2` of the
/// origin while the terms behind it are `2000`.
///
/// This rig exists because the chain brackets cannot be written on a
/// skinned joint. A displacement injected on one moves the composed
/// `W * B` as well as the world translation, and `SkinMatrix` — whose band
/// is derived from a different magnitude entirely — refuses it first. The
/// bracket then reports `kind: SkinMatrix` and its `expect_err` on
/// `RestTranslation` or `Trajectory` panics on the wrong kind, which looks
/// like a killed mutant and is not one: the chain band was never what
/// decided. Every earlier revision of the four brackets below did exactly
/// that, and four mutation rows were credited to fixtures that had not
/// detected them.
///
/// With `with_clip` the same bone carries an ordinary two-key translation
/// track whose value at `t = 0` is its own rest translation, so the
/// *sampled* chain cancels exactly as the rest chain does and the sampled
/// obligations run on a bone the skin still cannot see.
fn unskinned_sibling_document(with_clip: bool) -> Document {
    // The same local as `cancelling_chain_document`'s second joint —
    // `-R0^-1 (0, 1000, 0)` as literals — so this bone cancels against the
    // same parent world and needs no rotation of its own.
    let local = Vec3::new(483.7628, 749.96, 451.14697);
    let mut doc = cancelling_chain_document(1.0);
    doc.skeleton.bones.push(Bone {
        name: "unskinned_sibling".into(),
        parent: Some(1),
        rest: Transform {
            translation: local,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
        inverse_bind: None,
    });
    doc.assets.source_skeleton.nodes.push(SourceNodeAsset {
        source_node_index: 3,
        name: None,
        parent_source_node_index: Some(1),
        scene_root_indices: Vec::new(),
        local_rest: SourceNodeLocalRest::Trs {
            translation: local,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
        bone: Some(UNSKINNED_SIBLING_BONE),
    });
    if with_clip {
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: UNSKINNED_SIBLING_BONE,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![local, local * 2.0]),
            }],
        });
    }
    doc
}

/// [`unskinned_sibling_document`] under whole-document conversion at
/// `factor`, the only operation that puts the two documents' chain
/// magnitudes apart.
fn unskinned_sibling_conversion(
    with_clip: bool,
    factor: f64,
) -> (Document, ScalePlan, ScaleCandidate) {
    let doc = unskinned_sibling_document(with_clip);
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        document: &doc,
        capability: &capability,
    })
    .expect("a whole-document conversion plans at any positive factor");
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    (doc, plan, candidate)
}

/// The isolation each chain bracket rests on, asserted rather than
/// assumed: the defect's own bone is in the affected closure and in no
/// skin joint list, and a displacement along the same axis that the bands
/// admit leaves every skinned residual **bit-identical**.
///
/// An edit that wires this bone back into the skin — or a defect injected
/// somewhere that does move it — fails here, loudly, instead of silently
/// restoring the coincidental `SkinMatrix` death the brackets were written
/// to escape.
///
/// `perturbed` reads the field the caller displaced, off whichever
/// document it is handed. Bit-identical residuals are only evidence of
/// isolation if the two documents actually differ, and both ways of losing
/// that are silent: a caller that forgot to displace anything, and a
/// displacement small enough for `f32` to absorb — `+1e-3` on a stored
/// `1.54e6`, whose ulp is `0.125`, leaves the field unchanged and every
/// assertion below trivially true.
fn assert_the_defect_axis_is_invisible_to_the_skin(
    doc: &Document,
    plan: &ScalePlan,
    candidate: &ScaleCandidate,
    nudged: &ScaleCandidate,
    perturbed: impl Fn(&Document) -> f32,
) {
    assert_ne!(
        perturbed(candidate.document()),
        perturbed(nudged.document()),
        "the displacement did not survive storage, so the residuals below are equal because \
             the two documents are and this assertion proves nothing about isolation",
    );
    assert!(
        plan.affected_nodes().contains(&UNSKINNED_SIBLING_BONE),
        "the defect's bone is outside the affected closure, so no obligation compares it",
    );
    for instance in &doc.assets.instances {
        assert!(
            !instance.skin_joints.contains(&UNSKINNED_SIBLING_BONE),
            "the defect's bone is a skin joint again, so SkinMatrix and Bounds see the                  displacement and these brackets stop bracketing the chain term",
        );
    }
    let clean = prove_scale(doc, candidate, plan).expect("the correct candidate proves");
    let nudged =
        prove_scale(doc, nudged, plan).expect("a displacement the bands admit must still prove");
    assert_eq!(
        (
            clean.skin_matrix.max(),
            clean.skin_matrix.comparisons(),
            clean.bounds.max(),
            clean.bounds.comparisons(),
        ),
        (
            nudged.skin_matrix.max(),
            nudged.skin_matrix.comparisons(),
            nudged.bounds.max(),
            nudged.bounds.comparisons(),
        ),
        "the defect axis moved a skinned residual, so SkinMatrix can refuse it and these              brackets no longer isolate the chain term",
    );
}

/// Find an adjacent accepted/refused positive-binary32 transition.
///
/// The bit ordering of positive finite `f32` values is numeric ordering.
/// A coarse scan first guards the fixture's monotonicity assumption at
/// 4097 evenly spaced bit coordinates; it is not an exhaustive proof over
/// the interval. Bisection then returns one adjacent transition rather
/// than a guessed decimal bracket, and each caller pins its exact bits.
fn adjacent_positive_refusal(
    upper_refused: f32,
    mut refuses: impl FnMut(f32) -> bool,
) -> (f32, f32) {
    const MONOTONICITY_SAMPLES: u32 = 4096;
    let upper = upper_refused.to_bits();
    assert!(upper_refused.is_sign_positive());
    assert!(!refuses(0.0), "the unmodified candidate must prove");
    assert!(refuses(upper_refused), "the upper endpoint must refuse");

    let mut seen_refusal = false;
    for sample in 0..=MONOTONICITY_SAMPLES {
        let bits = (u64::from(upper) * u64::from(sample) / u64::from(MONOTONICITY_SAMPLES)) as u32;
        let refused = refuses(f32::from_bits(bits));
        assert!(
            !seen_refusal || refused,
            "the sampled refusal predicate reversed inside the searched positive-f32 interval"
        );
        seen_refusal |= refused;
    }

    let mut accepted = 0u32;
    let mut refused = upper;
    while accepted + 1 < refused {
        let middle = accepted + (refused - accepted) / 2;
        if refuses(f32::from_bits(middle)) {
            refused = middle;
        } else {
            accepted = middle;
        }
    }
    (f32::from_bits(accepted), f32::from_bits(refused))
}

fn residual_refusal(error: ScaleError, expected: ProofResidualKind) -> (f64, f64) {
    match error {
        ScaleError::ProofResidualExceeded {
            kind,
            observed,
            tolerance,
        } if kind == expected => (observed, tolerance),
        error => panic!("the floor stopped belonging to {expected:?}: {error:?}"),
    }
}

// Exact production floors measured on the merged v4 policy at
// 0a253228dc2d557a9030cfd72f2b15326f4853bd, with the same fixtures and
// mutation axes used below. These historical values are data, not a
// second supported policy implementation.
const V4_REST_TRAJECTORY_REFUSED_TRANSITION_BITS: u32 = 0x404c_0000;
const V4_SKIN_MATRIX_REFUSED_TRANSITION_BITS: u32 = 0x4064_91bb;
const V4_BOUNDS_REFUSED_TRANSITION_BITS: u32 = 0x403e_e3b7;

#[test]
fn the_historical_v4_floor_bits_are_pinned() {
    assert_eq!(
        [
            V4_REST_TRAJECTORY_REFUSED_TRANSITION_BITS,
            V4_SKIN_MATRIX_REFUSED_TRANSITION_BITS,
            V4_BOUNDS_REFUSED_TRANSITION_BITS,
        ],
        [0x404c_0000, 0x4064_91bb, 0x403e_e3b7],
        "the independently reproduced v4 floor data changed",
    );
}

/// A straight chain of `depth` bones under a root, each `(10, 0, 0)` from
/// its parent and carrying [`DEEP_CHAIN_ROTATION`].
///
/// The world translation accumulates one link's rounding per link. This
/// is the shape that separates "the worst case for one composition" from
/// "the worst case for a chain of them".
fn chain_document(depth: usize, rotation: Quat, root_scale: f32, animated: bool) -> Document {
    let mut nodes = vec![RigNode {
        parent: None,
        source_node_index: 0,
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(root_scale),
    }];
    for index in 1..=depth {
        nodes.push(RigNode {
            parent: Some(index - 1),
            source_node_index: index,
            translation: Vec3::new(10.0, 0.0, 0.0),
            rotation,
            scale: Vec3::ONE,
        });
    }
    let mut doc = rig_document(&nodes, &[depth], 0, Mat4::IDENTITY);
    if animated {
        doc.clips.push(Clip {
            name: "deep-chain".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::new(10.0, 0.0, 0.0),
                    Vec3::new(20.0, 0.0, 0.0),
                ]),
            }],
        });
    }
    doc
}

fn deep_chain_document(depth: usize) -> Document {
    chain_document(depth, DEEP_CHAIN_ROTATION, 1.0, false)
}

/// `170` degrees about `z`, as a literal for this section's reason:
/// `Quat::from_rotation_z` runs `f32` `sin`/`cos`, which is not
/// bit-identical across platforms, and the depth at which this rig crosses
/// the count is the thing being pinned.
///
/// Near a half turn is where each link's composed translation is smallest
/// relative to the terms it was summed from, so it is where a link
/// contributes the most rounding per unit of chain magnitude.
const DEEP_CHAIN_ROTATION: Quat = Quat::from_xyzw(0.0, 0.0, 0.996_194_7, 0.087_155_804);

/// `TAU / 192` about `z`, as a literal quaternion. Repeating this step
/// closes an ordinary ring/tread-shaped hierarchy at the depth the issue
/// originally named, without platform-dependent runtime trig.
const RING_CHAIN_ROTATION: Quat = Quat::from_xyzw(0.0, 0.0, 0.016_361_732, 0.999_866_1);

/// [`deep_chain_document`] converted at `1.5`, the operation that rewrites
/// every joint translation in the chain.
fn deep_chain_conversion(depth: usize) -> (Document, ScalePlan, ScaleCandidate) {
    let doc = deep_chain_document(depth);
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.5 },
        document: &doc,
        capability: &capability,
    })
    .expect("a whole-document conversion plans at any positive factor");
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    (doc, plan, candidate)
}

/// [`deep_chain_conversion`] with a two-key translation track whose first
/// sample is the authored rest local. At that sample the sampled world
/// walk is bit-for-bit the deep rest walk, so reverting only the sampled
/// provenance recurrence to a per-link `max` is isolated by Trajectory.
fn animated_deep_chain_conversion(depth: usize) -> (Document, ScalePlan, ScaleCandidate) {
    let doc = chain_document(depth, DEEP_CHAIN_ROTATION, 1.0, true);
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.5 },
        document: &doc,
        capability: &complete_capability(),
    })
    .expect("an animated whole-document chain plans");
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::Trajectories)
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    (doc, plan, candidate)
}

/// The raw ulp count [`ProofResidualKind::RestTranslation`] asked of its
/// own comparison base, in `calibrate_f32_rounding_ulps`'s units.
fn deep_chain_demand(depth: usize) -> (f64, Result<ScaleProof, ScaleError>) {
    let (doc, plan, candidate) = deep_chain_conversion(depth);
    let proof = prove_scale(&doc, &candidate, &plan);
    let demand = proof
        .as_ref()
        .map(|proof| proof.rest_translation_f32_rounding_demand)
        .unwrap_or_default();
    (demand, proof)
}

#[test]
fn accumulated_parent_chain_provenance_proves_through_depth_512() {
    let mut worst = 0.0f64;
    for depth in [8, 16, 32, 64, 128, 192, 256, 512] {
        let (demand, proof) = deep_chain_demand(depth);
        proof.unwrap_or_else(|error| {
            panic!(
                "a correct {depth}-link chain must prove with accumulated provenance: \
                        {error:?}"
            )
        });
        worst = worst.max(demand);
    }
    assert!(
        worst > 0.01 && worst < 1.0,
        "the declared deep-chain population no longer exercises the rounding term inside \
             its count: worst demand {worst}",
    );
}

#[test]
fn sampled_parent_chain_provenance_proves_through_depth_512() {
    for depth in [8, 16, 32, 64, 128, 192, 256, 512] {
        let (doc, plan, candidate) = animated_deep_chain_conversion(depth);
        let proof = prove_scale(&doc, &candidate, &plan).unwrap_or_else(|error| {
            panic!(
                "a correct sampled {depth}-link chain must prove with accumulated \
                        provenance: {error:?}"
            )
        });
        assert_eq!(proof.sample_time_count, 2);
        assert!(proof.trajectory.comparisons() > 0);
    }
}

#[test]
fn a_closed_loop_chain_proves_with_accumulated_provenance() {
    let doc = chain_document(192, RING_CHAIN_ROTATION, 1.0, true);
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.5 },
        document: &doc,
        capability: &complete_capability(),
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let source_pose = rest_world_pose(&doc.skeleton).unwrap();
    assert!(
        source_pose.bones[192].matrix.w_axis.truncate().length() < 1e-3,
        "the ring no longer closes, so it no longer exercises cancellation",
    );
    let proof =
        prove_scale(&doc, &candidate, &plan).expect("the correct closed-loop hierarchy must prove");
    assert_eq!(proof.sample_time_count, 2);
    assert!(proof.rest_translation.comparisons() > 0);
    assert!(proof.trajectory.comparisons() > 0);
    assert!(proof.skin_matrix.comparisons() > 0);
    assert!(proof.bounds.comparisons() > 0);
}

#[test]
fn translation_provenance_sums_only_spatial_link_operands() {
    let chain = |translations: &[f32]| {
        let mut bones = vec![Bone {
            name: "root".into(),
            parent: None,
            rest: Transform::default(),
            inverse_bind: None,
        }];
        for (index, &x) in translations.iter().enumerate() {
            bones.push(Bone {
                name: format!("bone{}", index + 1),
                parent: Some(index),
                rest: Transform {
                    translation: Vec3::new(x, 0.0, 0.0),
                    ..Transform::default()
                },
                inverse_bind: None,
            });
        }
        rest_world_pose(&Skeleton { bones }).unwrap()
    };

    let identity = chain(&vec![0.0; 512]);
    assert!(
        identity
            .bones
            .iter()
            .all(|pose| pose.translation_rounding_magnitude == 0.0),
        "the exact homogeneous row must not charge identity links",
    );

    let accumulating = chain(&[10.0, 10.0, 10.0]);
    assert_eq!(
        accumulating
            .bones
            .iter()
            .map(|pose| pose.translation_rounding_magnitude)
            .collect::<Vec<_>>(),
        [0.0, 10.0, 30.0, 60.0],
    );

    let uneven = chain(&[1.0, 2.0, 4.0]);
    assert_eq!(
        uneven
            .bones
            .iter()
            .map(|pose| pose.translation_rounding_magnitude)
            .collect::<Vec<_>>(),
        [0.0, 1.0, 4.0, 11.0],
        "each link must add its own accumulated parent/local base",
    );

    let one_cancellation = chain(&[1000.0, -1000.0, 0.0, 0.0, 0.0]);
    assert_eq!(
        one_cancellation
            .bones
            .iter()
            .map(|pose| pose.translation_rounding_magnitude)
            .collect::<Vec<_>>(),
        [0.0, 1000.0, 3000.0, 3000.0, 3000.0, 3000.0],
        "exact identity descendants must not turn one active cancellation into depth * max",
    );

    let scaled = rest_world_pose(&Skeleton {
        bones: vec![
            Bone {
                name: "scaled-root".into(),
                parent: None,
                rest: Transform {
                    scale: Vec3::splat(1000.0),
                    ..Transform::default()
                },
                inverse_bind: None,
            },
            Bone {
                name: "unit-link".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::X,
                    ..Transform::default()
                },
                inverse_bind: None,
            },
        ],
    })
    .unwrap();
    assert_eq!(
        scaled.bones[1].translation_rounding_magnitude, 1000.0,
        "provenance must sum the actual parent-world/local operand product, not local sizes",
    );

    let absorbed_parent = Mat4::from_translation(Vec3::new(2f32.powi(100), 0.0, 0.0));
    let absorbed_local = Mat4::from_translation(Vec3::new(2f32.powi(-100), 0.0, 0.0));
    let absorbed = translation_composition_rounding_base(absorbed_parent, absorbed_local);
    let contribution = 2f64.powi(-100);
    assert_eq!(
        absorbed,
        contribution + contribution / f64::from(f32::EPSILON),
        "an absorbed normal contribution must provision its own size, not the 2^100 parent",
    );

    let finite_parent = Mat4::from_cols(
        Vec4::new(2f32.powi(24), 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(1.0, 0.0, 0.0, 1.0),
    );
    let finite =
        translation_composition_rounding_base(finite_parent, Mat4::from_translation(Vec3::X));
    assert_eq!(
        finite,
        2f64.powi(24) + 1.0,
        "the finite parent/local sum must retain the one binary64 sees",
    );
    assert_eq!(
        finite as f32,
        2f32.powi(24),
        "binary32 would lose the local parent addend this provenance must retain",
    );
    let accumulated = child_translation_rounding_magnitude(
        WorldBonePose {
            matrix: finite_parent,
            translation_rounding_magnitude: 2f64.powi(24),
        },
        Mat4::from_translation(Vec3::X),
    );
    assert_eq!(
        accumulated,
        2f64.powi(25) + 1.0,
        "the chain accumulation itself must retain the binary64-only addend",
    );
    assert_eq!(
        accumulated as f32,
        2f32.powi(25),
        "binary32 chain accumulation would lose the same addend",
    );

    let at = |parent_scale: f32, local_x: f32| {
        translation_composition_rounding_base(
            Mat4::from_scale(Vec3::splat(parent_scale)),
            Mat4::from_translation(Vec3::new(local_x, 0.0, 0.0)),
        )
    };
    assert_eq!(
        at(1000.0, 0.25),
        250.0,
        "the link base must use the parent/local product, not either operand alone",
    );
    let minimum_subnormal = f32::from_bits(1);
    let below_minimum_subnormal = at(0.5, minimum_subnormal);
    let at_minimum_subnormal = at(1.0, minimum_subnormal);
    let below_minimum_normal = at(1.0, f32::from_bits(f32::MIN_POSITIVE.to_bits() - 1));
    let at_minimum_normal = at(1.0, f32::MIN_POSITIVE);
    assert!(
        0.0 < below_minimum_subnormal
            && below_minimum_subnormal < at_minimum_subnormal
            && at_minimum_subnormal < below_minimum_normal
            && below_minimum_normal < at_minimum_normal,
        "the subnormal floor must stay nonzero and monotone: \
             {below_minimum_subnormal:e}, {at_minimum_subnormal:e}, \
             {below_minimum_normal:e}, {at_minimum_normal:e}",
    );
    assert_eq!(
        at_minimum_subnormal,
        f64::from(f32::MIN_POSITIVE) + f64::from(minimum_subnormal),
        "the minimum-normal floor must provision one subnormal rounding step",
    );
}

#[test]
fn sampled_and_rest_provenance_match_on_the_z_spatial_row() {
    let local = Vec3::new(0.0, 0.0, 10.0);
    let skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::default(),
                inverse_bind: None,
            },
            Bone {
                name: "z-child".into(),
                parent: Some(0),
                rest: Transform {
                    translation: local,
                    ..Transform::default()
                },
                inverse_bind: None,
            },
        ],
    };
    let clip = Clip {
        name: "rest-equivalent-z".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![local, local]),
        }],
    };

    let rest = rest_world_pose(&skeleton).expect("the rest pose composes");
    let sampled = world_at_time(&skeleton, &clip, 0.0).expect("the sampled pose composes");
    assert_eq!(rest.bones[1].matrix, sampled.bones[1].matrix);
    assert_eq!(
        (
            rest.bones[1].translation_rounding_magnitude,
            sampled.bones[1].translation_rounding_magnitude,
        ),
        (10.0, 10.0),
        "rest and sampled poses must share the recurrence across all three spatial rows",
    );
}

#[test]
fn zero_translation_descendants_do_not_recharge_a_translated_parent() {
    let mut nodes = vec![RigNode {
        parent: None,
        source_node_index: 0,
        translation: Vec3::new(1_000_000.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }];
    for index in 1..=512 {
        nodes.push(RigNode {
            parent: Some(index - 1),
            source_node_index: index,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        });
    }
    let doc = rig_document(&nodes, &[512], 0, Mat4::IDENTITY);
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.5 },
        document: &doc,
        capability: &complete_capability(),
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let pose = rest_world_pose(&candidate.document().skeleton).unwrap();
    assert!(
        pose.bones
            .iter()
            .all(|bone| bone.translation_rounding_magnitude == 0.0),
        "copying a translated parent through zero locals is exact and must add no provenance",
    );

    let mut broken = candidate.document().clone();
    broken.skeleton.bones[512].rest.translation.x += 100.0;
    assert!(matches!(
        prove_scale(&doc, &ScaleCandidate { document: broken }, &plan),
        Err(ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::RestTranslation,
            ..
        })
    ));
}

#[test]
fn underflowed_translation_descendants_do_not_recharge_a_translated_parent() {
    let mut nodes = vec![RigNode {
        parent: None,
        source_node_index: 0,
        translation: Vec3::new(1_000_000.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(1e-30),
    }];
    for index in 1..=512 {
        nodes.push(RigNode {
            parent: Some(index - 1),
            source_node_index: index,
            translation: Vec3::new(f32::from_bits(1), 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        });
    }
    let doc = rig_document(&nodes, &[512], 0, Mat4::IDENTITY);
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.5 },
        document: &doc,
        capability: &complete_capability(),
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let pose = rest_world_pose(&candidate.document().skeleton).unwrap();
    assert!(
        pose.bones[512].translation_rounding_magnitude > 0.0
            && pose.bones[512].translation_rounding_magnitude < 1e-60,
        "products that binary32 rounds to zero may carry their tiny loss, but must not \
             recharge the million-unit parent: {}",
        pose.bones[512].translation_rounding_magnitude,
    );

    let mut broken = candidate.document().clone();
    broken.skeleton.bones[512].rest.translation.x = 1e32;
    assert!(matches!(
        prove_scale(&doc, &ScaleCandidate { document: broken }, &plan),
        Err(ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::RestTranslation,
            ..
        })
    ));
}

/// [`cancelling_chain_conversion`]'s rig with an ordinary two-key linear
/// translation track on the first joint, whose value at `t = 0` is that
/// joint's own rest translation — so the sampled parent chain cancels at
/// that sample exactly as the rest chain does, and the sampled obligations
/// run at all.
///
/// Without a track like this the only `Trajectory` comparison any fixture
/// in this section makes has `chain = 0` on both sides, which makes the
/// term arithmetically absent and every mutation of it a no-op.
fn cancelling_chain_clip_conversion_at(factor: f64) -> (Document, ScalePlan, ScaleCandidate) {
    let mut doc = cancelling_chain_document(1.0);
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![
                Vec3::new(0.0, 1000.0, 0.0),
                Vec3::new(0.0, 2000.0, 0.0),
            ]),
        }],
    });
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        document: &doc,
        capability: &capability,
    })
    .expect("a whole-document conversion plans at any positive factor");
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::Trajectories)
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    (doc, plan, candidate)
}

/// [`cancelling_chain_clip_conversion_at`] at `3190`.
fn cancelling_chain_clip_conversion() -> (Document, ScalePlan, ScaleCandidate) {
    cancelling_chain_clip_conversion_at(3190.0)
}

#[test]
fn a_sampled_pose_whose_parent_chain_cancels_still_proves_its_trajectory() {
    // `Trajectory` takes the same chain magnitude `RestTranslation` does,
    // read off the sampled pose rather than the rest pose, and this is the
    // fixture that makes that term do work. At `t = 0` the track drives the
    // first joint to its own rest translation, so the sampled chain cancels
    // exactly as the rest chain does and the sampled world translation is a
    // rounding artefact of `6.38e6`.
    //
    // Measured: the trajectory residual is `0.197` against `3.04` from the
    // candidate's sampled chain, `9.58e-4` from the source's, and `3.92e-6`
    // with no chain term. It is the rest residual to the digit, because at
    // this sample the two poses are the same pose — which is the point:
    // the two obligations differ only in which locals the chain ran on.
    let (doc, plan, candidate) = cancelling_chain_clip_conversion();
    let proof = prove_scale(&doc, &candidate, &plan)
        .expect("a correct candidate whose sampled chain cancels must still prove");
    assert_eq!(proof.sample_time_count, 2);
    let source_chain =
        rest_world_pose(&doc.skeleton).unwrap().bones[2].translation_rounding_magnitude;
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    assert!(
        proof.trajectory.max() > policy.f32_rounded_tolerance(0.0, 0.0, source_chain),
        "trajectory residual {} no longer exceeds what the source side's chain buys, so \
             this fixture no longer exercises the sampled chain term",
        proof.trajectory.max()
    );
}

/// The bone [`inherited_chain_document`] adds: a child of the cancellation
/// point, `1e-3` from it, and the only bone in the tree whose own link
/// composes on nothing while its ancestors composed on `6.38e6`.
const INHERITED_CHAIN_LEAF: BoneId = 3;

/// [`cancelling_chain_document`] with [`INHERITED_CHAIN_LEAF`] hanging off
/// the joint whose world translation cancelled.
///
/// Every other chain fixture compares the bone *at* the cancellation
/// point, where `translation_composition_rounding_base` on the link
/// contains the parent's own world translation and so already names the
/// terms that cancelled. The inherited parent provenance is redundant
/// there. One bone further down it is not: the leaf's parent
/// world translation is the `(0.125, 0, -0.125)` the cancellation left, so
/// the leaf's own link composes on `2.84` while the rounding its world
/// translation carries is still `6.38e6`'s.
fn inherited_chain_document() -> Document {
    let mut doc = cancelling_chain_document(1.0);
    let rest = Transform {
        translation: Vec3::new(1e-3, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    doc.skeleton.bones.push(Bone {
        name: "bone3".into(),
        parent: Some(2),
        rest,
        inverse_bind: None,
    });
    doc.assets.source_skeleton.nodes.push(SourceNodeAsset {
        source_node_index: 3,
        name: None,
        parent_source_node_index: Some(2),
        scene_root_indices: Vec::new(),
        local_rest: SourceNodeLocalRest::Trs {
            translation: rest.translation,
            rotation: rest.rotation,
            scale: rest.scale,
        },
        bone: Some(INHERITED_CHAIN_LEAF),
    });
    doc
}

/// [`inherited_chain_document`] under whole-document conversion at `3190`,
/// optionally carrying [`cancelling_chain_clip_conversion_at`]'s track so
/// the *sampled* walk runs too.
fn inherited_chain_conversion(animated: bool) -> (Document, ScalePlan, ScaleCandidate) {
    let mut doc = inherited_chain_document();
    if animated {
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::new(0.0, 1000.0, 0.0),
                    Vec3::new(0.0, 2000.0, 0.0),
                ]),
            }],
        });
    }
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 3190.0 },
        document: &doc,
        capability: &capability,
    })
    .expect("a whole-document conversion plans at any positive factor");
    assert_eq!(
        plan.obligations()
            .contains(&ScaleProofObligation::Trajectories),
        animated
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    (doc, plan, candidate)
}

/// The magnitude the leaf's *parent* composed on and the magnitude the
/// leaf's own link composes on, off `pose` — the two numbers
/// the recurrence carries into the leaf.
///
/// Deliberately read off bone `2`'s chain rather than the leaf's own. The
/// leaf's chain *is* the expression under test, so a shape assertion built
/// on it would fire when that expression is mutated and pre-empt the
/// refusal this fixture exists to catch. Bone `2`'s chain is the rig's own
/// geometry — it is that bone's per-link term either way — so a guard on
/// it says only what it means to say.
fn inherited_chain_terms(skeleton: &Skeleton, pose: &WorldPose) -> (f64, f64) {
    let parent = pose.bones[2];
    let local = skeleton.bones[INHERITED_CHAIN_LEAF].rest.to_mat4();
    let own_link = translation_composition_rounding_base(parent.matrix, local);
    (pose.bones[2].translation_rounding_magnitude, own_link)
}

#[test]
fn a_bone_below_a_cancelling_chain_inherits_its_parent_s_magnitude_and_still_proves() {
    // The fixture for the *inherited* half of the chain magnitude —
    // parent provenance in `rest_world_pose` — as
    // distinct from the per-link half every other chain fixture reaches.
    // The mutation it kills is the parent addend deleted, leaving each bone with
    // only `translation_composition_rounding_base` on its own link: without
    // this fixture that mutation survives the whole suite.
    //
    // It survives because every other chain fixture compares the bone at
    // the cancellation point, and there the per-link term already contains
    // the parent's world translation — the very terms that cancelled — so
    // the inherited value is not independently needed there. The
    // leaf here is one bone further down. Its parent's world translation is
    // the `(0.125, 0, -0.125)` the cancellation left behind, so its own
    // link composes on `2.84` while the `6.38e6` its world translation
    // still carries the rounding of reaches it only by inheritance.
    //
    // Measured: the candidate's chain is `6.38e6` at the leaf with the
    // inheritance and `2.84` without, against a residual of `0.197`. The
    // correct candidate below proves today and is refused by
    // `RestTranslation` at `observed: 0.19679` against
    // `tolerance: 3.4982e-5` with the parent addend removed — a false refusal by
    // `5600x`. Removing it at that one site kills two tests, this and the
    // sampled fixture below, whose rest obligation runs on the same rig;
    // before the two of them it killed nothing in the suite.
    let (doc, plan, candidate) = inherited_chain_conversion(false);
    let pose = rest_world_pose(&candidate.document().skeleton).unwrap();
    let (chain, own_link) = inherited_chain_terms(&candidate.document().skeleton, &pose);

    // The rig's shape is the fixture: a chain that stopped cancelling, or
    // a leaf whose own link grew to name its ancestors' magnitude, proves
    // below whether the inheritance is there or not.
    assert!(
        chain > 1e5 * own_link,
        "the leaf's own link now composes on {own_link} against a parent chain of {chain}, \
             so this fixture no longer separates the two halves of the chain magnitude",
    );

    let proof = prove_scale(&doc, &candidate, &plan).expect(
        "a correct candidate whose leaf sits below a cancelling chain must still prove: the \
             rounding its world translation carries is its ancestors', and only the inherited \
             chain magnitude names it",
    );
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    assert!(
        proof.rest_translation.max() > policy.f32_rounded_tolerance(0.0, 0.0, own_link),
        "rest translation residual {} no longer exceeds what the leaf's own link buys, so \
             dropping the inherited term would no longer refuse this correct candidate",
        proof.rest_translation.max(),
    );
}

#[test]
fn a_sampled_bone_below_a_cancelling_chain_inherits_its_parent_s_magnitude_and_still_proves() {
    // The same separation for the *sampled* walk's copy of the inherited
    // term, in `world_at_time` rather than `rest_world_pose`. The two are
    // separate lines of code, and the rest fixture above covers only the
    // first: removing the parent addend in `world_at_time` alone leaves every
    // other test in the suite green and kills this one, by `Trajectory`,
    // which is the only obligation that reads that walk's chain.
    //
    // On `cancelling_chain_clip_conversion_at`'s track for that fixture's
    // reason: at `t = 0` it drives the first joint to its own rest
    // translation, so the sampled chain cancels exactly as the rest chain
    // does and the sampled leaf inherits the same `6.38e6`.
    //
    // Measured: refused at `observed: 0.19679` against
    // `tolerance: 3.4982e-5` with the sampled `max` removed — the rest
    // fixture's numbers to the digit, because at this sample the two poses
    // are the same pose.
    let (doc, plan, candidate) = inherited_chain_conversion(true);
    let skeleton = &candidate.document().skeleton;
    let pose = world_at_time(skeleton, &candidate.document().clips[0], 0.0).unwrap();
    let (chain, own_link) = inherited_chain_terms(skeleton, &pose);
    assert!(
        chain > 1e5 * own_link,
        "the sampled leaf's own link now composes on {own_link} against a parent chain of \
             {chain}, so this fixture no longer separates the two halves of the sampled chain",
    );

    let proof = prove_scale(&doc, &candidate, &plan).expect(
        "a correct candidate whose sampled leaf sits below a cancelling chain must still \
             prove, for the rest fixture's reason",
    );
    assert_eq!(proof.sample_time_count, 2);
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    assert!(
        proof.trajectory.max() > policy.f32_rounded_tolerance(0.0, 0.0, own_link),
        "trajectory residual {} no longer exceeds what the sampled leaf's own link buys, so \
             dropping the inherited term in `world_at_time` would no longer refuse this correct \
             candidate",
        proof.trajectory.max(),
    );
}

#[test]
fn a_shrinking_conversion_holds_trajectory_to_the_candidate_s_own_chain() {
    // `a_shrinking_conversion_holds_rest_translation_to_the_candidate_s_own_chain`
    // for the sampled obligation, on the same rig with the same clip: at
    // `0.01` the sampled source chain is `2000` against the candidate's
    // `20`, so a `max` over the two sides is the source side and is loose
    // by `100x`.
    //
    // `3e-5` is the bracket, and it is a narrow one because `TrackValue`
    // closes in from above as the factor shrinks: it compares the key
    // values themselves, which are `10` units here, against a `1.01e-4`
    // band. Below that and above the `1.05e-5` the candidate's sampled
    // chain buys, `Trajectory` is the only obligation that can refuse this
    // candidate — and with the source side's `9.54e-4` it does not.
    //
    // Measured: with the `max`, the smallest refused track displacement at
    // `0.01` is `1.01e-4` and it is refused by `TrackValue`, not by
    // `Trajectory` — this obligation contributed nothing. With the
    // candidate's chain alone it is `1.04e-5`, refused by `Trajectory`.
    //
    // On `unskinned_sibling_document`'s track, so that the displacement
    // moves the sampled world translation of a bone the skin cannot see.
    // An earlier revision animated joint `1`, which is a skin joint: the
    // sampled `W * B` moved with it and `SkinMatrix` refused at `5.0e-5`
    // before `Trajectory`'s band decided anything.
    let (doc, plan, candidate) = unskinned_sibling_conversion(true, 0.01);
    let mut nudged = candidate.document().clone();
    let TrackValues::Vec3s(values) = &mut nudged.clips[0].tracks[0].values else {
        panic!("expected a vec3 track");
    };
    values[0].x += 1e-6;
    values[1].x += 1e-6;
    assert_the_defect_axis_is_invisible_to_the_skin(
        &doc,
        &plan,
        &candidate,
        &ScaleCandidate { document: nudged },
        |document: &Document| {
            let TrackValues::Vec3s(values) = &document.clips[0].tracks[0].values else {
                panic!("expected a vec3 track");
            };
            values[0].x
        },
    );

    let mut broken = candidate.document().clone();
    let TrackValues::Vec3s(values) = &mut broken.clips[0].tracks[0].values else {
        panic!("expected a vec3 track");
    };
    values[0].x += 3e-5;
    values[1].x += 3e-5;
    let broken = ScaleCandidate { document: broken };
    let error = prove_scale(&doc, &broken, &plan)
        .expect_err("a 3e-5 sampled displacement must be refused on the candidate's chain");
    let ScaleError::ProofResidualExceeded {
        kind: ProofResidualKind::Trajectory,
        observed,
        tolerance,
    } = error
    else {
        panic!("expected a refused trajectory, got {error:?}");
    };
    let source_chain = rest_world_pose(&doc.skeleton).unwrap().bones[UNSKINNED_SIBLING_BONE]
        .translation_rounding_magnitude;
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    assert!(
        observed > tolerance,
        "trajectory band moved: observed {observed}, tolerance {tolerance}"
    );
    assert!(
        observed < policy.f32_rounded_tolerance(0.0, 0.0, source_chain),
        "trajectory residual {observed} now exceeds what the source chain buys too, so \
             this fixture no longer kills the source-side reading",
    );
}

#[test]
fn the_trajectory_v6_floor_is_an_adjacent_f32_transition() {
    // The over-acceptance direction for `Trajectory`, as above. Both keys
    // move together so the displacement is present at every sample rather
    // than only at the cancelling one, which is the shape of a track that
    // was rebased in the wrong basis.
    //
    // The search ends at `10`: `100` is refused by `TrackValue` first —
    // that obligation compares the key values themselves against a `31.9`
    // band here — so a larger interval would stop isolating this
    // obligation. The helper samples for gross non-monotonicity and
    // returns an adjacent accepted/refused pair within that interval.
    //
    // On `unskinned_sibling_document`'s track for that rig's reason: this
    // fixture is the only killer of `Trajectory`'s chain magnitude `x 64`,
    // and on a skin joint it never gets to be — `SkinMatrix` refuses a
    // `10`-unit sampled displacement at `10.03` first.
    let (doc, plan, candidate) = unskinned_sibling_conversion(true, 3190.0);
    let refuses = |delta: f32| {
        let mut document = candidate.document().clone();
        let TrackValues::Vec3s(values) = &mut document.clips[0].tracks[0].values else {
            panic!("expected a vec3 track");
        };
        values[0].x += delta;
        values[1].x += delta;
        match prove_scale(&doc, &ScaleCandidate { document }, &plan) {
            Ok(_) => false,
            Err(ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::Trajectory,
                ..
            }) => true,
            Err(error) => panic!("the floor stopped belonging to Trajectory: {error:?}"),
        }
    };
    let (accepted, refused) = adjacent_positive_refusal(10.0, refuses);
    assert_eq!(
        (accepted.to_bits(), refused.to_bits()),
        (0x4092_0000, 0x4092_0001),
    );
    assert!(refused.to_bits() > V4_REST_TRAJECTORY_REFUSED_TRANSITION_BITS);

    let mut nudged = candidate.document().clone();
    let TrackValues::Vec3s(values) = &mut nudged.clips[0].tracks[0].values else {
        panic!("expected a vec3 track");
    };
    values[0].x += accepted;
    values[1].x += accepted;
    assert_the_defect_axis_is_invisible_to_the_skin(
        &doc,
        &plan,
        &candidate,
        &ScaleCandidate { document: nudged },
        |document: &Document| {
            let TrackValues::Vec3s(values) = &document.clips[0].tracks[0].values else {
                panic!("expected a vec3 track");
            };
            values[0].x
        },
    );
    let mut first_refused = candidate.document().clone();
    let TrackValues::Vec3s(values) = &mut first_refused.clips[0].tracks[0].values else {
        panic!("expected a vec3 track");
    };
    values[0].x += refused;
    values[1].x += refused;
    let (observed, tolerance) = residual_refusal(
        prove_scale(
            &doc,
            &ScaleCandidate {
                document: first_refused,
            },
            &plan,
        )
        .unwrap_err(),
        ProofResidualKind::Trajectory,
    );
    assert_eq!(
        (observed.to_bits(), tolerance.to_bits()),
        (0x4012_464d_554e_2270, 0x4012_40e6_ce11_6746),
        "the exact Trajectory floor measurement drifted",
    );
}

#[test]
fn a_parent_chain_whose_operand_sums_overflow_f32_still_proves() {
    // A changed row's ordinary operand sum runs past `f32::MAX` exactly
    // where cancellation has taken that magnitude *out* of the composed
    // world. Keeping the binary32 infinity would make every tolerance
    // derived from it infinite, which `check_residual` refuses; computing
    // the per-link base in binary64 must retain a finite value instead.
    let doc = cancelling_chain_overflow_document();
    let local = doc.skeleton.bones[2].rest.to_mat4();
    let worlds = world_rests(&doc.skeleton).unwrap();
    assert!(
        !(mat4_abs(worlds[1]) * local.w_axis.abs())
            .max_element()
            .is_finite(),
        "the fixture no longer overflows the f32 lane computation, so it no longer \
             exercises the fallback",
    );
    assert!(
        mat4_is_finite(worlds[2]),
        "the composed world must stay finite: an overflowing world is a different failure, \
             and one that is allowed to be refused",
    );

    let pose = rest_world_pose(&doc.skeleton).unwrap();
    assert!(
        pose.bones[2].translation_rounding_magnitude.is_finite(),
        "chain magnitude {} is not finite",
        pose.bones[2].translation_rounding_magnitude
    );

    let plan = rest_bind_plan(&doc, 1e3);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).expect(
        "a correct candidate whose chain operands overflow f32 must still prove: every \
             operand is finite and so is the world they compose to",
    );
    assert!(
        proof.skin_matrix.max().is_finite() && proof.bounds.max().is_finite(),
        "skin {} / bounds {} residual is not finite",
        proof.skin_matrix.max(),
        proof.bounds.max()
    );
}

/// `scalar_tolerance` at a single magnitude, for fixtures that assert a
/// residual exceeds the band the *component alone* would have bought.
fn policy_scalar_tolerance_at(magnitude: f64) -> f64 {
    ScaleTolerancePolicy::APPENDIX_D_V6.scalar_tolerance(magnitude, magnitude)
}

#[test]
fn the_bounds_v6_floor_is_an_adjacent_f32_transition() {
    // Isolate the cost #337 adds to Bounds. The sole point's million-unit
    // x coordinate makes MeshPosition's own relative band wider than the
    // searched y defect, while the cancelling joint chain still dominates
    // the transform-stage provenance. Requiring `Bounds` as the first
    // refusal proves neither MeshPosition nor SkinMatrix owns the floor.
    let mut doc = cancelling_chain_document(3190.0);
    let primitive = &mut doc.assets.meshes[0].primitives[0];
    primitive.positions = vec![Vec3::new(1_000_000.0, 0.0, 0.0)];
    primitive.joints = vec![[1, 0, 0, 0]];
    primitive.weights = vec![[1.0, 0.0, 0.0, 0.0]];
    let plan = rest_bind_plan(&doc, 3190.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    assert!(
        rig_slot_magnitude(candidate.document()) > 4_000_000.0,
        "the chain no longer dominates the point's transform base"
    );
    let refuses = |delta: f32| {
        let mut document = candidate.document().clone();
        document.assets.meshes[0].primitives[0].positions[0].y += delta;
        match prove_scale(&doc, &ScaleCandidate { document }, &plan) {
            Ok(_) => false,
            Err(ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::Bounds,
                ..
            }) => true,
            Err(error) => panic!("the floor stopped belonging to Bounds: {error:?}"),
        }
    };
    let (accepted, refused) = adjacent_positive_refusal(8.0, refuses);
    assert_eq!(
        (accepted.to_bits(), refused.to_bits()),
        (0x4090_1eeb, 0x4090_1eec),
    );
    assert!(refused.to_bits() > V4_BOUNDS_REFUSED_TRANSITION_BITS);
    let mut first_refused = candidate.document().clone();
    first_refused.assets.meshes[0].primitives[0].positions[0].y += refused;
    let (observed, tolerance) = residual_refusal(
        prove_scale(
            &doc,
            &ScaleCandidate {
                document: first_refused,
            },
            &plan,
        )
        .unwrap_err(),
        ProofResidualKind::Bounds,
    );
    assert_eq!(
        (observed.to_bits(), tolerance.to_bits()),
        (0x4012_40e6_6000_0000, 0x4012_40e6_54a0_13ff),
        "the exact Bounds floor measurement drifted",
    );
}

#[test]
fn the_skin_matrix_v6_floor_is_an_adjacent_f32_transition() {
    // Shift the inverse bind of the joint at the cancellation point. This
    // cannot affect rest, tracks or stored positions; SkinMatrix runs
    // before Bounds, so the typed refused endpoint isolates the chain-derived
    // skin band.
    let doc = cancelling_chain_document(3190.0);
    let plan = rest_bind_plan(&doc, 3190.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    assert!(
        rig_slot_magnitude(candidate.document()) > 1_000_000.0,
        "the cancelling chain no longer dominates the near-unit W * B product"
    );
    let refuses = |delta: f32| {
        let mut document = candidate.document().clone();
        document.assets.instances[0].skin_ibms[1].w_axis.x += delta;
        match prove_scale(&doc, &ScaleCandidate { document }, &plan) {
            Ok(_) => false,
            Err(ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::SkinMatrix,
                ..
            }) => true,
            Err(error) => panic!("the floor stopped belonging to SkinMatrix: {error:?}"),
        }
    };
    let (accepted, refused) = adjacent_positive_refusal(16.0, refuses);
    assert_eq!(
        (accepted.to_bits(), refused.to_bits()),
        (0x40ab_6d4b, 0x40ab_6d4c),
    );
    assert!(refused.to_bits() > V4_SKIN_MATRIX_REFUSED_TRANSITION_BITS);
    let mut first_refused = candidate.document().clone();
    first_refused.assets.instances[0].skin_ibms[1].w_axis.x += refused;
    let (observed, tolerance) = residual_refusal(
        prove_scale(
            &doc,
            &ScaleCandidate {
                document: first_refused,
            },
            &plan,
        )
        .unwrap_err(),
        ProofResidualKind::SkinMatrix,
    );
    assert_eq!(
        (observed.to_bits(), tolerance.to_bits()),
        (0x4012_40e6_6000_0000, 0x4012_40e6_40a0_13ff),
        "the exact SkinMatrix floor measurement drifted",
    );
}

#[test]
fn the_f32_rounding_term_is_absolute_so_it_cannot_widen_a_comparison_of_its_own_magnitude() {
    // The constraint the term is built to satisfy: where the compared
    // quantity *is* the magnitude the arithmetic ran on, the added term
    // is `f32_rounding_ulps * 2^-23` of it, which is twenty times below
    // the relative band `scalar_relative` already declares. Stated as a
    // ratio rather than as two literals so it holds at every magnitude
    // rather than at the one a fixture happened to pick.
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    for magnitude in [1e-6, 1.0, 3190.0, 1e9] {
        let plain = policy.scalar_tolerance(magnitude, magnitude);
        let rounded = policy.f32_rounded_tolerance(magnitude, magnitude, magnitude);
        let added = rounded - plain;
        assert!(
            added <= policy.scalar_relative * magnitude / 20.0,
            "at {magnitude} the rounding term {added} is not far below the relative band"
        );
    }
    // And it is absolute in the magnitude, not relative to it: a
    // comparison of a small component against a large magnitude gets the
    // large magnitude's ulp, which is the whole point.
    let small = policy.f32_rounded_tolerance(5.982, 5.982, 4242.0);
    assert!(
        small > 2.44e-4,
        "the reproducer's residual is not admitted: {small}"
    );
}

#[test]
fn an_overflowing_skinned_position_is_named_as_overflow_not_as_a_generic_non_finite() {
    // Overflow and `NaN` are different failures. An overflowing skinned
    // position is a document whose geometry leaves the `f32` range this
    // proof computes in; a `NaN` is a degenerate input that survived
    // every finiteness check. Reporting both as `non_finite_result` tells
    // an operator nothing about which one they have.
    //
    // Deliberately no magnitude domain is asserted alongside this.
    // `transform_point3` accumulates a dot product, so where it
    // overflows depends on the rotation and not on the magnitude of the
    // result — there is no constant a document could be checked against
    // ahead of time.
    let overflowing = [unrounded_slot(Mat4::from_scale(Vec3::splat(1e30)))];
    let mut accumulator = BoundsAccumulator::default();
    assert_eq!(
        accumulate_skinned_bounds(
            4,
            2,
            &Primitive {
                positions: vec![Vec3::splat(1e30)],
                joints: vec![[0, 0, 0, 0]],
                weights: vec![[1.0, 0.0, 0.0, 0.0]],
                ..Primitive::default()
            },
            &overflowing,
            &mut accumulator,
        ),
        Err(ScaleError::InvalidSkinnedPrimitive {
            instance_index: 4,
            primitive_index: 2,
            reason: "skinned_magnitude_overflow",
        })
    );

    // A `NaN` keeps the older, wider reason. `0 * inf` is `NaN`, so a
    // slot whose linear part overflows against a zero coordinate produces
    // one without any input being `NaN` itself.
    let nan_producing = [unrounded_slot(Mat4::from_cols(
        Vec4::new(f32::INFINITY, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    ))];
    let mut accumulator = BoundsAccumulator::default();
    assert_eq!(
        accumulate_skinned_bounds(
            4,
            2,
            &Primitive {
                positions: vec![Vec3::new(0.0, 1.0, 0.0)],
                joints: vec![[0, 0, 0, 0]],
                weights: vec![[1.0, 0.0, 0.0, 0.0]],
                ..Primitive::default()
            },
            &nan_producing,
            &mut accumulator,
        ),
        Err(ScaleError::InvalidSkinnedPrimitive {
            instance_index: 4,
            primitive_index: 2,
            reason: "non_finite_result",
        })
    );
}

#[test]
fn bounds_provenance_uses_the_per_axis_transform_and_slot_composition() {
    // The diagonal point pins the per-axis norm: every component ran on
    // `300`, so an L2 reading of the blended point would incorrectly
    // widen the base to `sqrt(3) * 300`. A slot that cancelled two large
    // terms still carries that composition provenance even when its
    // skinned point is near the origin.
    let primitive = Primitive {
        positions: vec![Vec3::splat(300.0)],
        joints: vec![[0, 0, 0, 0]],
        weights: vec![[1.0, 0.0, 0.0, 0.0]],
        ..Primitive::default()
    };

    let mut points_dominate = BoundsAccumulator::default();
    accumulate_skinned_bounds(
        0,
        0,
        &primitive,
        &[unrounded_slot(Mat4::IDENTITY)],
        &mut points_dominate,
    )
    .unwrap();
    assert!((points_dominate.rounding_magnitude() - 300.0).abs() < 1e-6);

    // `W = [I | (5000, 0, 0)]` against `B = W^-1`: the product is the
    // identity, so `matrix_magnitude` of it is `1`, but the translation
    // column was the difference of two `5000`s.
    let world = Mat4::from_translation(Vec3::new(5000.0, 0.0, 0.0));
    let mut composition_dominates = BoundsAccumulator::default();
    accumulate_skinned_bounds(
        0,
        0,
        &primitive,
        &[SkinSlot::compose(world, world.inverse(), 0.0)],
        &mut composition_dominates,
    )
    .unwrap();
    assert!(
        composition_dominates.rounding_magnitude() >= 5000.0,
        "the composition's magnitude was lost: {}",
        composition_dominates.rounding_magnitude()
    );
}

#[test]
fn a_tiny_influence_carries_only_its_proportional_bounds_provenance() {
    // Isolate the slot-composition half: both composed matrices are the
    // identity, but the second was produced at magnitude `1e20`. Its
    // `1e-20` weight contributes about `1`, rather than buying the full
    // distant slot's tolerance. The transform-operand half is `1` for
    // both influences, so an unweighted max or a mispaired
    // `max(weight) * max(magnitude)` fails this numeric pin.
    let primitive = Primitive {
        positions: vec![Vec3::ZERO],
        joints: vec![[0, 1, 0, 0]],
        weights: vec![[1.0, 1e-20, 0.0, 0.0]],
        ..Primitive::default()
    };
    let slots = [
        SkinSlot {
            matrix: Mat4::IDENTITY,
            absolute: Mat4::IDENTITY,
            rounding_magnitude: 1.0,
        },
        SkinSlot {
            matrix: Mat4::IDENTITY,
            absolute: Mat4::IDENTITY,
            rounding_magnitude: 1e20,
        },
    ];
    let mut accumulator = BoundsAccumulator::default();
    accumulate_skinned_bounds(0, 0, &primitive, &slots, &mut accumulator).unwrap();

    let magnitude = accumulator.rounding_magnitude();
    assert!(
        (magnitude - 2.0).abs() < 1e-6,
        "the tiny slot bought more than its proportional provenance: {magnitude}"
    );
}

#[test]
fn a_tiny_influence_carries_only_its_proportional_transform_provenance() {
    // The transform itself, not the slot-composition history, is huge in
    // this half. Both weighted contributions are `1`, so retaining the
    // lightly weighted transform's unweighted `1e20` operand magnitude
    // would be an unbounded tolerance widening.
    let primitive = Primitive {
        positions: vec![Vec3::new(1.0, 0.0, 0.0)],
        joints: vec![[0, 1, 0, 0]],
        weights: vec![[1.0, 1e-20, 0.0, 0.0]],
        ..Primitive::default()
    };
    let slots = [
        SkinSlot {
            matrix: Mat4::IDENTITY,
            absolute: Mat4::IDENTITY,
            rounding_magnitude: 1.0,
        },
        SkinSlot {
            matrix: Mat4::from_scale(Vec3::splat(1e20)),
            absolute: Mat4::from_scale(Vec3::splat(1e20)),
            rounding_magnitude: 1.0,
        },
    ];
    let mut accumulator = BoundsAccumulator::default();
    accumulate_skinned_bounds(0, 0, &primitive, &slots, &mut accumulator).unwrap();

    let magnitude = accumulator.rounding_magnitude();
    assert!(
        (magnitude - 2.0).abs() < 1e-5,
        "the tiny transform bought more than its proportional provenance: {magnitude}"
    );
}

#[test]
fn weighted_bounds_provenance_recovers_the_tiny_influence_detection_floor() {
    // The second joint sits at `1e20`, but its composed slot moves the
    // vertex by only `1e17` and its weight is `1e-20`. The source bound is
    // therefore about `1e-3`. Dropping that influence in the candidate is
    // a real bound defect above the ordinary scalar band.
    //
    // Under v3's unweighted max the far joint bought an `O(1e20)` base, so
    // the complete band admitted `9.53e13` units and this defect proved.
    // The normalized provenance is about `2`, so v4 refuses it. Both
    // numbers are read through production helpers to make this a measured
    // before/after detection floor rather than a restatement of the
    // formula.
    let doc = composed_slot_document(
        [Quat::IDENTITY; 2],
        1.0,
        [Vec3::ZERO, Vec3::new(1e20, 0.0, 0.0)],
        [
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(1e17, 0.0, 0.0)),
        ],
        &[Vec3::ZERO],
        &[[1.0, 1e-20, 0.0, 0.0]],
    );
    let plan = rest_bind_plan(&doc, 1.0);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    prove_scale(&doc, &candidate, &plan).expect("the valid tiny influence must prove");

    let slots = rig_skin_slots(&doc);
    let old_unweighted_base = slots
        .iter()
        .map(|slot| slot.rounding_magnitude)
        .fold(0.0, f64::max);
    let weighted_base = rig_bounds_magnitude(&doc);
    assert!(
        old_unweighted_base >= 1e20 && weighted_base < 3.0,
        "fixture did not separate old and weighted bases: {old_unweighted_base} / \
             {weighted_base}"
    );

    let mut broken = candidate.document().clone();
    broken.assets.meshes[0].primitives[0].weights[0][1] = 0.0;
    let error = prove_scale(&doc, &ScaleCandidate { document: broken }, &plan)
        .expect_err("dropping the tiny influence must no longer hide behind the far joint");
    let ScaleError::ProofResidualExceeded {
        kind: ProofResidualKind::Bounds,
        observed,
        tolerance,
    } = error
    else {
        panic!("expected the bounds obligation, got {error:?}");
    };
    let old_unweighted_tolerance =
        ScaleTolerancePolicy::APPENDIX_D_V6.f32_rounded_tolerance(0.0, 0.0, old_unweighted_base);
    assert!(
        observed > tolerance && observed < old_unweighted_tolerance,
        "detection floor did not tighten: observed {observed}, v4 {tolerance}, old \
             unweighted {old_unweighted_tolerance}"
    );

    // Record the exact before/after detection floor with one candidate
    // mutation and adjacent positive binary32 values. The historical
    // predicate is a frozen copy of v3's Bounds arithmetic and policy, so
    // a future v4 edit cannot silently rewrite the historical baseline.
    fn v3_bounds(document: &Document) -> ((Vec3, Vec3), f64) {
        let slots = rig_skin_slots(document);
        let instance = &document.assets.instances[0];
        let mesh = &document.assets.meshes[instance.mesh];
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut magnitude = 0.0f64;
        for primitive in &mesh.primitives {
            for (vertex, &position) in primitive.positions.iter().enumerate() {
                let joints = primitive.joints[vertex];
                let weights = primitive.weights[vertex];
                let mut skinned = Vec3::ZERO;
                let mut weight_sum = 0.0f32;
                let mut vertex_magnitude = 0.0f64;
                for slot_index in 0..4 {
                    let weight = weights[slot_index];
                    if weight == 0.0 {
                        continue;
                    }
                    let slot = &slots[joints[slot_index] as usize];
                    skinned += weight * slot.matrix.transform_point3(position);
                    weight_sum += weight;
                    vertex_magnitude = vertex_magnitude.max(column_operand_magnitude(
                        slot.absolute,
                        position.extend(1.0),
                    ));
                    vertex_magnitude = vertex_magnitude.max(slot.rounding_magnitude);
                }
                if weight_sum > 0.0 {
                    skinned /= weight_sum;
                    min = min.min(skinned);
                    max = max.max(skinned);
                    let length = skinned.length();
                    let length = if length.is_finite() {
                        f64::from(length)
                    } else {
                        skinned.as_dvec3().length()
                    };
                    magnitude = magnitude.max(vertex_magnitude).max(length);
                }
            }
        }
        ((min, max), magnitude)
    }

    fn v3_bounds_outcome(
        source: (Vec3, Vec3),
        candidate: (Vec3, Vec3),
        magnitude: f64,
    ) -> (bool, f64) {
        let mut refused = false;
        let mut max_residual = 0.0f64;
        for (before, after) in [(source.0, candidate.0), (source.1, candidate.1)] {
            for (before, after) in before.to_array().into_iter().zip(after.to_array()) {
                let expected = f64::from(before);
                let observed = f64::from(after);
                let residual = (observed - expected).abs();
                let tolerance = 1e-6
                    + 1e-5 * expected.abs().max(observed.abs())
                    + 4.0 * magnitude.abs() * f64::from(f32::EPSILON);
                refused |= residual > tolerance;
                max_residual = max_residual.max(residual);
            }
        }
        (refused, max_residual)
    }

    fn adjacent_transition(
        accepted_value: f32,
        refused_value: f32,
        mut refuses: impl FnMut(f32) -> bool,
    ) -> (f32, f32) {
        let mut accepted = accepted_value.to_bits();
        let mut refused = refused_value.to_bits();
        assert!(!refuses(f32::from_bits(accepted)));
        assert!(refuses(f32::from_bits(refused)));
        while accepted + 1 < refused {
            let middle = accepted + (refused - accepted) / 2;
            if refuses(f32::from_bits(middle)) {
                refused = middle;
            } else {
                accepted = middle;
            }
        }
        (f32::from_bits(accepted), f32::from_bits(refused))
    }

    fn adjacent_reverse_transition(
        refused_value: f32,
        accepted_value: f32,
        mut refuses: impl FnMut(f32) -> bool,
    ) -> (f32, f32) {
        let mut refused = refused_value.to_bits();
        let mut accepted = accepted_value.to_bits();
        assert!(refuses(f32::from_bits(refused)));
        assert!(!refuses(f32::from_bits(accepted)));
        while refused + 1 < accepted {
            let middle = refused + (accepted - refused) / 2;
            if refuses(f32::from_bits(middle)) {
                refused = middle;
            } else {
                accepted = middle;
            }
        }
        (f32::from_bits(refused), f32::from_bits(accepted))
    }

    let (source_v3_bounds, source_v3_magnitude) = v3_bounds(&doc);
    let original_weight = 1e-20f32;
    let mutate = |far_weight: f32| {
        let mut broken = candidate.document().clone();
        broken.assets.meshes[0].primitives[0].weights[0][1] = far_weight;
        broken
    };
    let v4_refuses = |far_weight: f32| {
        let broken = ScaleCandidate {
            document: mutate(far_weight),
        };
        match prove_scale(&doc, &broken, &plan) {
            Ok(_) => false,
            Err(ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::Bounds,
                ..
            }) => true,
            Err(error) => panic!("the v4 floor reached another obligation: {error:?}"),
        }
    };
    let (v4_lower_refused, v4_lower_accepted) =
        adjacent_reverse_transition(0.0, original_weight, v4_refuses);
    let (v4_upper_accepted, v4_upper_refused) =
        adjacent_transition(original_weight, 1.0, v4_refuses);
    let (v3_accepted, v3_refused) = adjacent_transition(original_weight, 1.0, |far_weight| {
        let broken = mutate(far_weight);
        let (candidate_v3_bounds, candidate_v3_magnitude) = v3_bounds(&broken);
        let v3_magnitude = candidate_v3_magnitude.max(source_v3_magnitude);
        assert_eq!(
            v3_magnitude, old_unweighted_base,
            "the historical base moved at candidate weight {far_weight}"
        );
        v3_bounds_outcome(source_v3_bounds, candidate_v3_bounds, v3_magnitude).0
    });
    let zero_v3 = v3_bounds(&mutate(0.0));
    assert!(
        !v3_bounds_outcome(
            source_v3_bounds,
            zero_v3.0,
            source_v3_magnitude.max(zero_v3.1),
        )
        .0,
        "v3 unexpectedly had a lower transition"
    );

    let v4_refusal = |far_weight: f32| {
        let broken = ScaleCandidate {
            document: mutate(far_weight),
        };
        let error = prove_scale(&doc, &broken, &plan).unwrap_err();
        let ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::Bounds,
            observed,
            tolerance,
        } = error
        else {
            panic!("the v4 endpoint reached another obligation: {error:?}");
        };
        (observed, tolerance)
    };
    let v4_lower_outcome = v4_refusal(v4_lower_refused);
    let v4_upper_outcome = v4_refusal(v4_upper_refused);
    let v4_floor = v4_lower_outcome.0.min(v4_upper_outcome.0);
    let (v3_refused_bounds, v3_refused_magnitude) = v3_bounds(&mutate(v3_refused));
    let v3_floor = v3_bounds_outcome(
        source_v3_bounds,
        v3_refused_bounds,
        source_v3_magnitude.max(v3_refused_magnitude),
    )
    .1;

    assert_eq!(
        [
            v4_lower_refused.to_bits(),
            v4_lower_accepted.to_bits(),
            v4_upper_accepted.to_bits(),
            v4_upper_refused.to_bits(),
            v3_accepted.to_bits(),
            v3_refused.to_bits(),
        ],
        [
            0x1e3c_6f0a,
            0x1e3c_6f0b,
            0x1e3d_5b21,
            0x1e3d_5b22,
            0x3a7a_1be3,
            0x3a7a_1be4,
        ],
        "the recorded before/after weight brackets moved; recalibrate and update \
         DESIGN.md Appendix D section D.1"
    );
    assert_eq!(v4_lower_refused.to_bits() + 1, v4_lower_accepted.to_bits());
    assert_eq!(v4_upper_accepted.to_bits() + 1, v4_upper_refused.to_bits());
    assert_eq!(v3_accepted.to_bits() + 1, v3_refused.to_bits());
    assert_eq!(v4_floor.to_bits(), 0x3ec4_7800_0000_0000);
    assert_eq!(v3_floor.to_bits(), 0x42d5_ac65_4000_0000);
    assert!(
        v3_floor / v4_floor > 3.9e19,
        "the weighted base recovered too little detection power: v3 residual {v3_floor}, \
             v4 residual {v4_floor}"
    );
}

#[test]
fn the_fourth_skin_influence_of_a_vertex_is_walked_like_the_first_three() {
    // glTF's `JOINTS_0`/`WEIGHTS_0` carry exactly four influences per
    // vertex, and `accumulate_skinned_bounds` walks `0..4`. No other
    // fixture in this file gives a vertex a nonzero *fourth* influence, so
    // a walk that stopped at three would still pass every one of them —
    // while silently dropping a legal influence from the bounds both sides
    // of the proof are measured on.
    //
    // Dropping it symmetrically is invisible: the same truncated walk runs
    // over the source and the candidate, so an equally-wrong pair of
    // bounds still agree. The defect below is therefore one only the
    // fourth slot carries — a candidate that rebound slot 3 to slot 0's
    // joint. `validate_candidate_structure` compares mesh identity, skin
    // joints and per-primitive vertex counts, but not per-vertex joint
    // indices, and the skin-matrix obligation compares `W * B` per
    // `skin_joints` slot, which this leaves untouched: the bounds walk is
    // the only thing that can see it.
    //
    // Every joint is a direct child of the root, so with a whole-document
    // factor of `0.01` the four rest-world skin matrices (identity binds)
    // are pure translations `0.01 * (0, k, 0)` for `k = 1..4`, and the one
    // vertex is evenly blended across all four:
    //
    //   source skinned    = 0.25 * (W1 + W2 + W3 + W4) * p
    //   candidate skinned = 0.25 * (W1 + W2 + W3 + W1) * p
    //   difference        = 0.25 * 0.01 * (0, 1 - 4, 0) = (0, -0.0075, 0)
    //
    // against a bounds tolerance of `1e-6 + 1e-5 * O(1)`, so the
    // rejection is four orders of magnitude clear of the band.
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        rig(Some(0), 2, Vec3::new(0.0, 2.0, 0.0)),
        rig(Some(0), 3, Vec3::new(0.0, 3.0, 0.0)),
        rig(Some(0), 4, Vec3::new(0.0, 4.0, 0.0)),
    ];
    let mut doc = rig_document(&nodes, &[1, 2, 3, 4], 0, Mat4::IDENTITY);
    doc.assets.meshes[0].primitives[0] = Primitive {
        positions: vec![Vec3::new(1.0, 0.0, 0.0)],
        joints: vec![[0, 1, 2, 3]],
        weights: vec![[0.25, 0.25, 0.25, 0.25]],
        ..Primitive::default()
    };
    assert_eq!(doc.assets.instances[0].skin_joints, vec![1, 2, 3, 4]);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::SkinAndBounds)
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    // The undoctored candidate proves, so the rejection below is the
    // rebound influence and nothing about the fixture.
    prove_scale(&doc, &candidate, &plan).unwrap();

    let mut broken = candidate.document().clone();
    broken.assets.meshes[0].primitives[0].joints[0] = [0, 1, 2, 0];
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::Bounds,
            ..
        }
    ));
}

#[test]
fn rest_bind_rebases_translation_tracks_and_proves_every_sampled_obligation() {
    let doc = multi_joint_document();
    let capability = complete_capability();
    let plan = multi_joint_plan(&doc, &capability);
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    let obligations = plan.obligations().to_vec();
    assert!(obligations.contains(&ScaleProofObligation::KeyTranslations));
    assert!(obligations.contains(&ScaleProofObligation::CubicInteriors));
    assert!(obligations.contains(&ScaleProofObligation::Trajectories));
    assert!(obligations.contains(&ScaleProofObligation::SkinAndBounds));

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let clip = &candidate.document().clips[0];

    // Both tracks sit on a node whose parent is itself affected, so the
    // parent-basis multiplier of DESIGN.md Appendix D §D.2 is `s = 0.01`
    // for values *and* both cubic tangents.
    let TrackValues::Vec3s(linear) = &clip.tracks[0].values else {
        panic!("expected a vec3 track");
    };
    let expected_linear = [Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 2.0, 0.0)];
    for (value, expected) in linear.iter().zip(expected_linear) {
        assert!((*value - expected).length() < 1e-6, "{value:?}");
    }
    let TrackValues::Vec3s(cubic) = &clip.tracks[1].values else {
        panic!("expected a vec3 track");
    };
    let expected_cubic = [
        Vec3::ZERO,
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.6, 0.0),
        Vec3::new(0.0, 0.6, 0.0),
        Vec3::new(0.0, 3.0, 0.0),
        Vec3::ZERO,
    ];
    for (value, expected) in cubic.iter().zip(expected_cubic) {
        assert!((*value - expected).length() < 1e-6, "{value:?}");
    }

    // Both rebased binds are hand-derived: `B' = C^-1 * B = scale(s) * B`.
    let binds = &candidate.document().assets.instances[0].skin_ibms;
    assert!(binds[0].abs_diff_eq(Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)), 1e-5));
    assert!(binds[1].abs_diff_eq(Mat4::from_translation(Vec3::new(0.0, -2.0, 0.0)), 1e-5));

    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    // Two key times plus the analytic midpoint of the one cubic segment.
    assert_eq!(proof.sample_time_count, 3);
    assert!(proof.key_translation.max() < 1e-4);
    assert!(proof.cubic_interior.max() < 1e-4);
    assert!(proof.trajectory.max() < 1e-4);
    assert!(proof.skin_matrix.max() < 1e-4);
    assert!(proof.bounds.max() < 1e-4);
}

#[test]
fn a_reweighted_vertex_is_named_by_the_bounds_obligation() {
    // Per-vertex skin weights are the one rewritten-document payload no
    // other obligation reads: they do not appear in a track value, a
    // base `POSITION`, a world matrix, or `W * B`. At rest both joint
    // palettes are the identity, so this candidate is only distinguished
    // once the clip drives the two joints apart.
    let doc = multi_joint_document();
    let capability = complete_capability();
    let plan = multi_joint_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut broken = candidate.document().clone();
    broken.assets.meshes[0].primitives[0].weights[2] = [0.75, 0.25, 0.0, 0.0];
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::Bounds,
            ..
        }
    ));
}

// --- Capability gate ---------------------------------------------------

/// One row of the capability-gate table: the flag's name and the single
/// mutation it applies to an otherwise complete projection.
type CapabilityDomainCase = (&'static str, fn(&mut ScaleCapabilityFacts));

#[test]
fn every_unsupported_capability_domain_rejects_planning_on_its_own() {
    // DESIGN.md Appendix D §D.4: every unmodeled domain fails closed.
    // One flag at a time, against an otherwise complete projection, so a
    // dropped clause cannot hide behind a sibling.
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let domains: [CapabilityDomainCase; 12] = [
        ("cameras_present", |f| f.cameras_present = true),
        ("lights_present", |f| f.lights_present = true),
        ("instancing_present", |f| f.instancing_present = true),
        ("unregistered_extensions_present", |f| {
            f.unregistered_extensions_present = true
        }),
        ("extras_present", |f| f.extras_present = true),
        ("unknown_source_members_present", |f| {
            f.unknown_source_members_present = true
        }),
        ("non_triangle_primitives_present", |f| {
            f.non_triangle_primitives_present = true
        }),
        ("unsupported_vertex_attributes_present", |f| {
            f.unsupported_vertex_attributes_present = true
        }),
        ("secondary_skin_influences_present", |f| {
            f.secondary_skin_influences_present = true
        }),
        ("inverse_bind_issues_present", |f| {
            f.inverse_bind_issues_present = true
        }),
        ("unsafe_accessor_layout_present", |f| {
            f.unsafe_accessor_layout_present = true
        }),
        ("external_resources_present", |f| {
            f.external_resources_present = true
        }),
    ];
    let operations = [
        ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
    ];
    for (name, set_flag) in domains {
        let mut capability = complete_capability();
        set_flag(&mut capability);
        assert!(!capability.is_supported(), "{name} must not be supported");
        for operation in operations {
            let request = ScaleRequest {
                operation,
                document: &doc,
                capability: &capability,
            };
            assert!(
                matches!(
                    plan_scale(&request).unwrap_err(),
                    ScaleError::IncompleteCapability
                ),
                "{name} must reject {operation:?}"
            );
        }
    }
    // The complete projection these were derived from is genuinely
    // supported, so each rejection above is attributable to its own flag.
    assert!(complete_capability().is_supported());
}

#[test]
fn morph_capabilities_are_whole_document_only() {
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    for (name, set_flag) in [
        (
            "morphs_present",
            (|facts: &mut ScaleCapabilityFacts| facts.morphs_present = true)
                as fn(&mut ScaleCapabilityFacts),
        ),
        (
            "morph_weights_present",
            |facts: &mut ScaleCapabilityFacts| {
                facts.morph_weights_present = true;
            },
        ),
    ] {
        let mut capability = complete_capability();
        set_flag(&mut capability);
        assert!(
            !capability.is_supported(),
            "the operation-agnostic query remains conservative for {name}"
        );

        assert_eq!(
            plan_scale(&ScaleRequest {
                operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
                document: &doc,
                capability: &capability,
            })
            .unwrap_err(),
            ScaleError::IncompleteCapability,
            "presence without a raw preservation witness must reject {name}"
        );
        capability.whole_document_morphs_preservable = true;
        let whole_document = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &doc,
            capability: &capability,
        };
        assert!(
            plan_scale(&whole_document).is_ok(),
            "raw format adapters may discharge {name} for whole-document conversion"
        );

        let rest_bind = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        };
        assert_eq!(
            plan_scale(&rest_bind).unwrap_err(),
            ScaleError::IncompleteCapability,
            "rest/bind has no raw morph preservation proof for {name}"
        );
    }
}

// --- Tolerance policy identity -----------------------------------------

#[test]
fn the_appendix_d_v6_tolerance_identity_is_pinned_through_plan_and_proof() {
    // DESIGN.md Appendix D §D.1/§D.6: producers record this identity and
    // these thresholds in evidence, so a change to either is a new policy
    // identity rather than a silent retune.
    fn assert_appendix_d_v6(policy: ScaleTolerancePolicy) {
        assert_eq!(policy.id, "appendix-d-v6");
        assert_eq!(policy.f32_rounding_ulps, 4);
        assert_eq!(policy.relative_orthogonality, 1e-5);
        assert_eq!(policy.equal_axis, 1e-5);
        assert_eq!(policy.common_factor, 1e-5);
        assert_eq!(policy.singular_determinant_relative, 1e-6);
        assert_eq!(policy.scalar_absolute, 1e-6);
        assert_eq!(policy.scalar_relative, 1e-5);
        assert_eq!(policy.rotation_residual_radians, 1e-5);
        // `2^-14` exactly: the unit-scale bound v2 declared and v3/v4
        // retain, on the binary32 mantissa grid the composed-scale
        // measurement lives on.
        assert_eq!(policy.postcondition_unit_scale_residual, 6.103_515_625e-5);
        assert_eq!(policy.proof_sample_work_budget, 400_000_000);
        // `abs_error <= 1e-6 + 1e-5 * max(abs(before), abs(after))`, at a
        // hand-computed operand pair.
        assert!((policy.scalar_tolerance(0.0, 100.0) - 0.001_001).abs() < 1e-12);
        // The unit-scale bound is *derived* from the common-factor band,
        // not declared independently: four bands is `4e-5`, and the
        // declared bound is the next power of two at or above it, which
        // is also an exact multiple of `2^-23` and therefore a value the
        // binary32 composed-scale measurement can equal. Pinning every
        // half of that sentence keeps a future retune of either number
        // from silently reopening the window §D.1 closes.
        let bands = ScaleTolerancePolicy::UNIT_SCALE_BANDS * policy.common_factor;
        assert!((bands - 4e-5).abs() < 1e-20, "four bands {bands}");
        assert!(policy.postcondition_unit_scale_residual >= bands);
        assert_eq!(policy.postcondition_unit_scale_residual, 2f64.powi(-14));
        assert_eq!(
            policy.postcondition_unit_scale_residual,
            512.0 * 2f64.powi(-23)
        );
        // Rounding onto that grid is a rounding, not an open-ended
        // loosening: the declared bound is the *next* power of two at or
        // above four bands, so halving it must fall below them. That
        // pins "rounded up to the next power of two" exactly, rather
        // than merely bounding the result from one side.
        assert!(policy.postcondition_unit_scale_residual / 2.0 < bands);
        // Three of the four bands are analytic — the declared-factor
        // match, the mixed-factor match, and the equal-axis match, each
        // contributing at most `c / (1 - c)` — so the composed analytic
        // worst case is `(1 - c)^-3 - 1`. The declared bound has to sit
        // strictly above it with a whole band left over, which is what
        // makes the fourth band genuine float headroom.
        let c = policy.common_factor;
        // `(1 - x)^-3 - 1 = 3x + 6x^2 + 10x^3 + ...`, so at `x = 1e-5`
        // the first two terms are `3.00006e-5` and everything after them
        // is below `1.1e-14`.
        let analytic_worst_case = (1.0 - c).powi(-3) - 1.0;
        assert!(
            (analytic_worst_case - 3.00006e-5).abs() < 1e-13,
            "three composed bands {analytic_worst_case}"
        );
        assert!(
            policy.postcondition_unit_scale_residual - analytic_worst_case > c,
            "headroom {} is under one band",
            policy.postcondition_unit_scale_residual - analytic_worst_case
        );
    }

    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();

    let whole_document = whole_document_plan(&doc, &capability);
    assert_appendix_d_v6(whole_document.tolerance_policy());
    let candidate = build_scale_candidate(&doc, &whole_document).unwrap();
    let proof = prove_scale(&doc, &candidate, &whole_document).unwrap();
    assert_appendix_d_v6(proof.tolerance_policy);

    let rest_bind = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    assert_appendix_d_v6(rest_bind.tolerance_policy());
    let candidate = build_scale_candidate(&doc, &rest_bind).unwrap();
    let proof = prove_scale(&doc, &candidate, &rest_bind).unwrap();
    assert_appendix_d_v6(proof.tolerance_policy);
}

// --- Tolerance boundary and reported maxima ----------------------------

#[test]
fn a_residual_exactly_at_its_tolerance_is_accepted_and_the_next_one_up_is_not() {
    // DESIGN.md Appendix D §D.1 states every residual is "at most" its
    // bound, i.e. the bound is *inclusive*. `check_residual` therefore
    // rejects on `observed > tolerance`, and nothing else in the module
    // distinguishes that from `observed >= tolerance` unless a comparison
    // actually lands on the bound. `next_up` is the immediately larger
    // `f64`, so the accept/reject pair below straddles the bound with no
    // representable value in between.
    let bound = ScaleTolerancePolicy::APPENDIX_D_V6.postcondition_unit_scale_residual;
    assert_eq!(
        check_residual(ProofResidualKind::UnitScale, bound, bound),
        Ok(())
    );
    let above = f64::from_bits(bound.to_bits() + 1);
    assert_eq!(
        check_residual(ProofResidualKind::UnitScale, above, bound),
        Err(ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::UnitScale,
            observed: above,
            tolerance: bound,
        })
    );
}

#[test]
fn a_unit_scale_residual_exactly_on_the_policy_bound_still_proves() {
    // The same inclusive boundary, reached end to end rather than by
    // calling the comparison directly. This is the reason the v2 policy
    // states its unit-scale bound as `2^-14`: the measured residual is
    // `after_scale - 1` for a binary32 `after_scale`, hence always an
    // integer multiple of `2^-23` near unit magnitude, and `2^-14` is
    // `512 * 2^-23`. A bound off that grid could never be *met*, only
    // undershot, and the inclusive/exclusive distinction would be
    // untestable.
    //
    // The source is a rig whose whole affected domain has an exact
    // rest-world factor of `0.5`, so `plan_scale` accepts with a zero
    // factor residual and every other obligation below is analytically
    // exact:
    //
    //   W0 = W1 = diag(0.5),  B1 = diag(2),  W1 * B1 = I
    //
    // The candidate is then hand-built to sit exactly on the bound: its
    // root local scale is `1 + 2^-14` (exactly representable), and its
    // inverse bind is `diag(1 - 2^-14)`. Their product rounds to exactly
    // `1.0` in binary32 — `(1 + 2^-14)(1 - 2^-14) = 1 - 2^-28`, and
    // `2^-28` is far below the `2^-24` half-ulp below one — so the skin
    // equation and the skinned bounds both still hold exactly while the
    // composed scale is off by exactly the declared bound.
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.5),
        },
        rig(Some(0), 1, Vec3::ZERO),
    ];
    let doc = rig_document(&nodes, &[1], 0, Mat4::from_scale(Vec3::splat(2.0)));
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.5,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::RestWorldAndUnitScale)
    );

    let bound = plan.tolerance_policy().postcondition_unit_scale_residual;
    let mut boundary = build_scale_candidate(&doc, &plan).unwrap().into_document();
    boundary.skeleton.bones[0].rest.scale = Vec3::splat(1.0 + 2.0f32.powi(-14));
    boundary.assets.instances[0].skin_ibms[0] =
        Mat4::from_scale(Vec3::splat(1.0 - 2.0f32.powi(-14)));
    let boundary = ScaleCandidate { document: boundary };

    let proof = prove_scale(&doc, &boundary, &plan).unwrap();
    assert_eq!(proof.unit_scale.max(), bound);
    assert_eq!(proof.unit_scale.max(), 2f64.powi(-14));
}

#[test]
fn a_successful_proof_reports_the_residual_maxima_it_actually_observed() {
    // `ScaleProof`'s residual fields are exactly what DESIGN.md Appendix
    // D §D.6 requires producer evidence to serialize, so a proof that
    // succeeded while reporting `0.0` for a residual that was really
    // `2.2e-10` would publish a false record. Accept/reject is unaffected
    // by that confusion, so only an assertion on a *nonzero* maximum from
    // a *successful* proof can catch it.
    //
    // Every value below is the same single quantity, hand-computed:
    // the whole-document builder narrows the declared `0.01` to binary32
    // before multiplying, while proof forms its expectation in `f64`.
    //
    //   0.01_f32 = 10737418 * 2^-30 = 0.00999999977648258209228515625
    //   0.01_f64 = 0.010000000000000000208166817117216851...
    //   residual = 0.01_f64 - 0.01_f32 = 2.2351741811588166e-10
    //
    // and the tolerance it passes is `1e-6 + 1e-5 * 0.01 = 1.0001e-6`.
    //
    //   rest translation: child rest `(0, 1, 0)` -> `(0, 0.01_f32, 0)`
    //   mesh position:    vertex `(1, 0, 0)` -> `(0.01_f32, 0, 0)`
    //   bounds:           the one weighted vertex skins to `(1, 1, 0)`
    //                     in the source and `(0.01_f32, 0.01_f32, 0)` in
    //                     the candidate, and min == max
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();

    let expected = 0.01_f64 - (0.01_f32 as f64);
    assert_eq!(expected, 2.235_174_181_158_816_6e-10);
    assert_eq!(proof.rest_translation.max(), expected);
    assert_eq!(proof.mesh_position.max(), expected);
    assert_eq!(proof.bounds.max(), expected);
    // The skin equation, by contrast, is genuinely exact here: proof
    // forms its expectation with the same binary32 `scale_translation_only`
    // the builder used, so there is nothing to round. Pinning it at zero
    // keeps the three nonzero maxima above attributable.
    assert_eq!(proof.skin_matrix.max(), 0.0);
    assert!(proof.skin_matrix.evaluated());
}

#[test]
fn the_reported_unit_scale_residual_is_the_maximum_not_the_last_node_seen() {
    // `ScaleProof::unit_scale.max()` is a *maximum* over the affected
    // nodes, and #284 will freeze it as a published evidence field, so a
    // proof that reported the last node's residual instead of the largest
    // would publish a smaller number than it observed — accept/reject
    // unchanged, record false. This one and `rest_rotation.max()` are
    // the two checked against a fixed policy bound rather than a
    // before/after-derived tolerance, so they reach the shared fold
    // through `record_and_check` rather than `check_and_track`; for each
    // of them only a fixture whose maximum is *not* at the last affected
    // node can tell a maximum from a last-write — see
    // `the_reported_rest_rotation_residual_is_the_maximum_not_the_last_node_seen`
    // for the other.
    //
    // `plan.affected_nodes` is ascending `BoneId` order, so the rig puts
    // the larger residual on node 0. With `u = 2^-17 = 64 * 2^-23` and a
    // declared factor of `0.5`, every product below is exact in binary32:
    //
    //   root local scale  = 0.5 * (1 + u)          -> s_0 = 0.5 * (1 + u)
    //   child local scale = 1 - u/2
    //   child world       = 0.5 * (1 + u) * (1 - u/2)
    //                     = 0.5 * (1 + u/2)        (the `2^-36` term is
    //                                               below the `2^-24` ulp)
    //
    // The mixed-factor band sees `|A_1 - s_0| = 0.5 * u/2` against
    // `1e-5 * 0.5 * (1 + u)`, so it accepts. The candidate's root local
    // scale is `0.5 * (1 + u) * 2 = 1 + u`, hence
    //
    //   node 0 residual = u   = 64 * 2^-23 = 7.62939453125e-6
    //   node 1 residual = u/2 = 32 * 2^-23 = 3.814697265625e-6
    let u = 2f32.powi(-17);
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.5 * (1.0 + u)),
        },
        RigNode {
            parent: Some(0),
            source_node_index: 1,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(1.0 - u * 0.5),
        },
    ];
    let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.5,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    // The order the maximum is folded in: node 0 first, node 1 last.
    assert_eq!(plan.affected_nodes(), &[0, 1]);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.unit_scale.max(), 64.0 * 2f64.powi(-23));
    // Stated as its own inequality so the assertion above cannot be read
    // as "whatever the last node happened to produce".
    assert!(proof.unit_scale.max() > 32.0 * 2f64.powi(-23));
}

#[test]
fn each_clip_is_proved_against_its_own_sample_times() {
    // `prove_scale` harvests each clip's sample times once, up front, into
    // an index-parallel cache, then reads `clip_times[clip_index]` inside
    // the per-clip loop. That index is new in this revision — the times
    // used to be derived inside the loop, where they could not be wrong —
    // and reading clip 0's times while proving clip 1 is invisible to
    // every fixture whose clips share a timeline.
    //
    // The two clips here deliberately do not. Both drive bone 1, which
    // together with bone 2 skins the single vertex `(1, 0, 0)` at
    // `0.5 / 0.5`:
    //
    //   clip 0  times {0, 1}     bone 1 at the origin throughout
    //   clip 1  times {0, 1, 2}  bone 1 at the origin at 0 and 1,
    //                            and at (0, 10, 0) at 2
    //
    // At every time in clip 0 — and at clip 1's first two — both joint
    // palettes are the identity, so the vertex skins to `(1, 0, 0)`
    // whatever its weights are. Only at `t = 2` do the palettes differ:
    //
    //   source     0.5 * (1, 10, 0) + 0.5 * (1, 0, 0) = (1, 5, 0)
    //   candidate  1.0 * (1, 10, 0)                   = (1, 10, 0)
    //
    // a `5.0` bounds residual against a `1e-6 + 1e-5 * 10` tolerance.
    // Sampling clip 1 at `{0, 1}` clamps its track to its first key and
    // never reaches that pose, so the defect goes unseen and the proof
    // succeeds.
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::ZERO),
        rig(Some(0), 2, Vec3::ZERO),
    ];
    let mut doc = rig_document(&nodes, &[1, 2], 0, Mat4::IDENTITY);
    doc.assets.meshes[0].primitives[0] = Primitive {
        positions: vec![Vec3::new(1.0, 0.0, 0.0)],
        joints: vec![[0, 1, 0, 0]],
        weights: vec![[0.5, 0.5, 0.0, 0.0]],
        ..Primitive::default()
    };
    let clip = |name: &str, times: Vec<f32>, values: Vec<Vec3>| Clip {
        name: name.into(),
        duration_s: f64::from(*times.last().expect("at least one key time")),
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times,
            values: TrackValues::Vec3s(values),
        }],
    };
    doc.clips = vec![
        clip("still", vec![0.0, 1.0], vec![Vec3::ZERO; 2]),
        clip(
            "lift",
            vec![0.0, 1.0, 2.0],
            vec![Vec3::ZERO, Vec3::ZERO, Vec3::new(0.0, 10.0, 0.0)],
        ),
    ];

    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.0 },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // Two key times from the first clip and three from the second, not
    // twice the first clip's two.
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.sample_time_count, 5);

    let mut broken = candidate.document().clone();
    broken.assets.meshes[0].primitives[0].weights[0] = [1.0, 0.0, 0.0, 0.0];
    let broken = ScaleCandidate { document: broken };
    assert!(matches!(
        prove_scale(&doc, &broken, &plan).unwrap_err(),
        ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::Bounds,
            ..
        }
    ));
}

// --- Sampling budget ---------------------------------------------------

/// A whole-document fixture sized in sample times and skinned vertices,
/// so a test can name the exact `sample_times * per_sample_work_units`
/// work its document demands.
///
/// Two bones, one skinned instance with one slot, one primitive, and one
/// `Linear` translation track — which contributes `times.len()` key times
/// and no cubic-segment interiors. Its per-sample cost is therefore
/// `2 * 2 bones + (2 + 1) * 1 slot + 2 * vertices = 7 + 2 * vertices`.
fn budget_document(key_times: usize, vertices: usize) -> Document {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.assets.meshes[0].primitives[0] = Primitive {
        positions: (0..vertices)
            .map(|v| Vec3::new(v as f32 * 0.5, 1.0, 0.0))
            .collect(),
        joints: vec![[0, 0, 0, 0]; vertices],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; vertices],
        ..Primitive::default()
    };
    let times: Vec<f32> = (0..key_times).map(|i| i as f32 / 1000.0).collect();
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: f64::from(*times.last().expect("at least one key time")),
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times,
            values: TrackValues::Vec3s(vec![Vec3::new(0.0, 1.0, 0.0); key_times]),
        }],
    });
    doc
}

#[test]
fn a_document_above_the_sampling_budget_is_refused_with_a_typed_error() {
    // Hand-computed against
    // `ScaleTolerancePolicy::proof_sample_work_budget`:
    //
    //   per-sample cost = 2 sides * 2 bones                  =     4
    //                   + (2 sides + 1 skin residual) * 1 slot =    3
    //                   + 2 sides * 1000 skinned vertices    = 2_000
    //                                                          -------
    //                                                            2_007
    //   sample times    = 200_000 keys, no cubic segments
    //   work            = 200_000 * 2_007 = 401_400_000
    //   budget          =                   400_000_000
    //
    // The refusal is typed and total: proof never silently samples a
    // subset, and it is raised before the first sample time is evaluated.
    let doc = budget_document(200_000, 1_000);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    assert_eq!(
        prove_scale(&doc, &candidate, &plan).unwrap_err(),
        ScaleError::ProofSamplingBudgetExceeded {
            policy_id: "appendix-d-v6",
            sample_times: 200_000,
            per_sample_cost: 2_007,
            work: 401_400_000,
            budget: 400_000_000,
        }
    );
}

#[test]
fn a_representative_in_budget_document_still_proves() {
    // The same shape at production proportions — a 240-key clip over a
    // 2000-vertex skinned mesh — costs
    // `240 * (4 + 3 + 2 * 2000) = 240 * 4_007 = 961_680` work units, two
    // orders of magnitude inside the budget, and proves.
    let doc = budget_document(240, 2_000);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    // The vertices really are walked by the combined skin/bounds
    // obligation that charges for them.
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::SkinAndBounds)
    );
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.sample_time_count, 240);
}

/// Three bones; one mesh whose **two** primitives carry 5 and 7 vertices;
/// three instances, one of which repeats a joint across slots and one of
/// which is skinned entirely outside the affected closure `{1, 2}`.
fn work_unit_document() -> Document {
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
    ];
    let mut doc = rig_document(&nodes, &[1, 2], 0, Mat4::IDENTITY);
    let primitive = |vertices: usize| Primitive {
        positions: vec![Vec3::new(1.0, 0.0, 0.0); vertices],
        joints: vec![[0, 0, 0, 0]; vertices],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; vertices],
        ..Primitive::default()
    };
    doc.assets.meshes[0].primitives = vec![primitive(5), primitive(7)];
    let instance = |skin_joints: Vec<BoneId>| MeshInstance {
        source_node_index: 2,
        node: 1,
        mesh: 0,
        skin_ibms: vec![Mat4::IDENTITY; skin_joints.len()],
        skin_joints,
    };
    doc.assets.instances = vec![
        // Three slots, two of which name the *same* joint: legal input,
        // and the shape that makes "slot work cannot exceed the bone
        // count" false.
        instance(vec![1, 1, 2]),
        instance(vec![2]),
        // Outside the affected closure, so neither obligation walks it.
        instance(vec![0]),
    ];
    doc
}

#[test]
fn per_sample_work_units_charges_every_slot_primitive_and_document_side() {
    // Hand-computed, term by term, against what `check_skin_and_bounds`
    // actually performs at one sample time. The affected closure is
    // `{1, 2}`, so the third instance is scanned once while deriving the
    // working set and excluded from every sampled walk.
    //
    //   bones      2 sides * 3 bones                            =  6
    //   instance 0 3 slots: 2 sides                             =  6
    //              3 slots: 1 skin residual each                =  3
    //              (5 + 7) vertices * 2 sides                   = 24
    //   instance 1 1 slot:  2 sides                             =  2
    //              1 slot:  1 skin residual                     =  1
    //              (5 + 7) vertices * 2 sides                   = 24
    //
    // The undercount this replaces charged `bones + vertices` — `3 + 24`,
    // once — and dropped the slot term entirely on the false claim that
    // it "cannot exceed the bone count": instance 0 alone owes 9 units of
    // slot work against a 3-bone skeleton.
    let doc = work_unit_document();
    let affected: BTreeSet<BoneId> = [1, 2].into_iter().collect();
    let affected_skin_instances = affected_skin_instance_indices(&doc, &affected);
    assert_eq!(affected_skin_instances, vec![0, 1]);
    assert_eq!(per_sample_work_units(&doc, &affected_skin_instances), 66);
    // Without the obligation the caller passes an empty working set, so
    // only the two forward-kinematics passes remain.
    assert_eq!(per_sample_work_units(&doc, &[]), 6);
}

#[test]
fn per_sample_work_units_isolates_bone_slot_and_vertex_terms() {
    // Start with only the two skeleton walks, then add three skin slots with
    // no vertices, then add one five-vertex primitive while keeping the
    // skeleton and slots fixed. These named boundaries keep the aggregate
    // fixture above from hiding a missing both-sides multiplier in one term.
    let mut doc = work_unit_document();
    doc.assets.instances.truncate(1);
    doc.assets.meshes[0].primitives.clear();
    assert_eq!(
        per_sample_work_units(&doc, &[]),
        6,
        "two sides times three bones"
    );
    assert_eq!(
        per_sample_work_units(&doc, &[0]),
        15,
        "bone term 6 + slot products on two sides 6 + slot residuals 3"
    );
    doc.assets.meshes[0].primitives.push(Primitive {
        positions: vec![Vec3::ZERO; 5],
        joints: vec![[0; 4]; 5],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; 5],
        ..Primitive::default()
    });
    assert_eq!(
        per_sample_work_units(&doc, &[0]),
        25,
        "the five-vertex term adds exactly two sides times five"
    );
}

#[test]
fn sampled_proof_classifies_unrelated_skin_palettes_once() {
    let mut doc = compensated_document_with_unrelated_skin(Some(Mat4::IDENTITY));
    doc.assets.instances[1].skin_joints = vec![3; 10_000];
    doc.assets.instances[1].skin_ibms = vec![Mat4::IDENTITY; 10_000];
    doc.clips.push(Clip {
        name: "three-samples".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 0.5, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::new(0.0, 100.0, 0.0); 3]),
        }],
    });

    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    reset_affected_skin_classification_steps();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();

    // The affected instance matches at its first and only slot. The
    // unrelated instance examines all 10,000 repeated outside-closure
    // slots exactly once while the proof derives its working set. The
    // rest walk and three sampled walks then reuse that set.
    assert_eq!(affected_skin_classification_steps(), 10_001);
    assert_eq!(proof.sample_time_count, 3);
    assert_eq!(proof.skin_matrix.comparisons(), 4);
}

#[test]
fn a_document_whose_skin_slots_dominate_its_work_is_refused() {
    // The shape the old `bones + vertices` charge undercounted without
    // bound: many instances, many slots each, almost no vertices. Nothing
    // in `validate_scale_input` limits either count — it only range-checks
    // joint ids — so `skin_joints` may repeat a joint and an instance list
    // may be arbitrarily long.
    //
    //   per-sample cost = 2 sides * 2 bones                     =      4
    //                   + 50 instances * 100 slots * 2 sides    = 10_000
    //                   + 50 instances * 100 slots * 1 residual =  5_000
    //                   + 50 instances * 1 vertex * 2 sides     =    100
    //                                                             -------
    //                                                             15_104
    //   sample times    = 26_484 keys, no cubic segments
    //   work            = 26_484 * 15_104 = 400_014_336
    //   budget          =                   400_000_000
    //
    // `26_484` is the *first* key count this document cannot afford:
    // `26_483 * 15_104 = 399_999_232` is inside the budget. The old charge
    // read the same document as `2 bones + 50 vertices = 52` units per
    // sample — `1_377_168` work, 0.34% of the budget — while proof went on
    // to perform `26_484 * 50 * 100 * 2 = 264_840_000` slot matrix
    // products.
    let mut doc = budget_document(26_484, 1);
    let instance = MeshInstance {
        source_node_index: 1,
        node: 1,
        mesh: 0,
        skin_joints: vec![1; 100],
        skin_ibms: vec![Mat4::IDENTITY; 100],
    };
    doc.assets.instances = vec![instance; 50];
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert!(
        plan.obligations()
            .contains(&ScaleProofObligation::SkinAndBounds)
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    assert_eq!(
        prove_scale(&doc, &candidate, &plan).unwrap_err(),
        ScaleError::ProofSamplingBudgetExceeded {
            policy_id: "appendix-d-v6",
            sample_times: 26_484,
            per_sample_cost: 15_104,
            work: 400_014_336,
            budget: 400_000_000,
        }
    );
}

#[test]
fn cubic_segment_interior_times_count_toward_the_sampling_budget() {
    // `prove_scale` evaluates every cubic segment's analytic interior in
    // addition to its keys, so the budget has to count both. A
    // `CubicSpline` track with `k` keys contributes `k` key times and
    // `k - 1` interiors.
    //
    //   per-sample cost = 2 sides * 2 bones                     =      4
    //                   + 1 slot * (2 sides + 1 skin residual)  =      3
    //                   + 2 sides * 10_000 vertices             = 20_000
    //                                                             -------
    //                                                             20_007
    //   sample times    = 9_998 keys + 9_997 interiors = 19_995
    //   work            = 19_995 * 20_007 = 400_039_965
    //   budget          =                   400_000_000
    //
    // Counting keys alone would report `9_998` sample times here — and
    // `9_998 * 20_007 = 200_029_986`, comfortably inside the budget, so
    // the document would be sampled rather than refused.
    let mut doc = budget_document(9_998, 10_000);
    let keys = doc.clips[0].tracks[0].times.len();
    assert_eq!(keys, 9_998);
    doc.clips[0].tracks[0].interpolation = Interpolation::CubicSpline;
    doc.clips[0].tracks[0].values = TrackValues::Vec3s(vec![Vec3::new(0.0, 1.0, 0.0); keys * 3]);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    assert_eq!(
        prove_scale(&doc, &candidate, &plan).unwrap_err(),
        ScaleError::ProofSamplingBudgetExceeded {
            policy_id: "appendix-d-v6",
            sample_times: 19_995,
            per_sample_cost: 20_007,
            work: 400_039_965,
            budget: 400_000_000,
        }
    );
}

#[test]
fn the_sampling_budget_is_a_ceiling_a_document_may_reach() {
    // DESIGN.md Appendix D §D.1 states every policy quantity as an
    // inclusive "at most", so the budget comparison is `>` and a document
    // whose work lands exactly on the budget is sampled, not refused.
    // Nothing else in the module distinguishes `>` from `>=` unless a
    // document's work lands exactly on the ceiling, and a document that
    // does costs `4e8` work units to prove. Pinned here on synthetic
    // factors instead: `check_sampling_budget` takes the two numbers and
    // the policy, and nothing about the comparison depends on where they
    // came from.
    //
    // `400_000_000 = 20_000 * 20_000`, so the exact-ceiling pair is
    // literal, and `20_001 * 20_000 = 400_020_000` is the next
    // representable step up in `sample_times` — one more sample time of
    // the same document.
    let tol = ScaleTolerancePolicy::APPENDIX_D_V6;
    assert_eq!(tol.proof_sample_work_budget, 400_000_000);
    assert_eq!(check_sampling_budget(&tol, 20_000, 20_000), Ok(()));
    assert_eq!(
        check_sampling_budget(&tol, 20_001, 20_000),
        Err(ScaleError::ProofSamplingBudgetExceeded {
            policy_id: "appendix-d-v6",
            sample_times: 20_001,
            per_sample_cost: 20_000,
            work: 400_020_000,
            budget: 400_000_000,
        })
    );
    // One unit over, reached from the other factor, so neither operand's
    // role is assumed: `400_000_001 = 400_000_001 * 1`.
    assert_eq!(
        check_sampling_budget(&tol, 400_000_001, 1),
        Err(ScaleError::ProofSamplingBudgetExceeded {
            policy_id: "appendix-d-v6",
            sample_times: 400_000_001,
            per_sample_cost: 1,
            work: 400_000_001,
            budget: 400_000_000,
        })
    );
}

#[test]
fn an_overflowing_work_product_saturates_instead_of_wrapping_under_the_budget() {
    // The budget is `sample_times * per_sample_cost` in `u64`, and neither
    // factor is bounded by anything but the document. `saturating_mul` is
    // what makes the overflow fail *closed*: a wrapping product of two
    // enormous factors lands on a small number that sails under the
    // ceiling, which is precisely the refusal this check exists to make.
    //
    // `2^63 * 2 = 2^64`, which is `0` when wrapped — the most hostile case
    // there is, because zero passes any ceiling. Synthetic factors, for
    // the same reason `the_sampling_budget_is_a_ceiling_a_document_may_reach`
    // uses them: `check_sampling_budget` takes the two numbers and the
    // policy, and nothing about the arithmetic depends on where they came
    // from.
    let tol = ScaleTolerancePolicy::APPENDIX_D_V6;
    let sample_times = 9_223_372_036_854_775_808; // 2^63
    assert_eq!(
        check_sampling_budget(&tol, sample_times, 2),
        Err(ScaleError::ProofSamplingBudgetExceeded {
            policy_id: "appendix-d-v6",
            sample_times,
            per_sample_cost: 2,
            work: u64::MAX,
            budget: 400_000_000,
        })
    );
}

#[test]
fn duplicate_key_and_interior_times_across_tracks_are_charged_and_sampled_once() {
    // `clip_sample_times` harvests every affected track's key times into
    // one list and every cubic segment's interior into another, and both
    // are sorted and deduplicated. Two tracks on affected bones sharing a
    // time do not make proof evaluate that time twice, and — because
    // `sample_times` is one of the two factors `check_sampling_budget`
    // multiplies — must not be charged twice either.
    //
    // Two `CubicSpline` tracks, on two different affected bones, carrying
    // the *same* four key times (cloned, so they agree bit for bit):
    //
    //   keys      = {0, 0.001, 0.002, 0.003}                     = 4
    //   interiors = {0.0005, 0.0015, 0.0025}                     = 3
    //                                                              ---
    //   sample times                                                7
    //
    // Undeduplicated the same clip yields `8` keys and `6` interiors, so
    // dropping either `dedup` moves the count off `7`: `11` without the
    // key dedup, `10` without the interior dedup, `14` without both. No
    // interior coincides with a key, so the two lists never overlap and
    // `7` is not an accident of one list absorbing the other.
    let mut doc = budget_document(4, 1);
    let track = &mut doc.clips[0].tracks[0];
    assert_eq!(track.bone, 1);
    track.interpolation = Interpolation::CubicSpline;
    track.values = TrackValues::Vec3s(vec![Vec3::new(0.0, 1.0, 0.0); 4 * 3]);
    let mut twin = track.clone();
    twin.bone = 0;
    doc.clips[0].tracks.push(twin);

    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    // Both bones the two tracks target are inside the affected domain, so
    // both tracks really are harvested and the duplication is real.
    assert_eq!(plan.affected_nodes(), &[0, 1]);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.sample_time_count, 7);
}

#[test]
fn a_candidate_skeleton_may_not_carry_bones_the_budget_never_charged() {
    // `per_sample_work_units` measures the *source* skeleton, but
    // `sample_time_obligations` calls `world_at_time` on **both**, and
    // `ScaleCandidate::from_document` is public — so without the
    // `bone_count_mismatch` parity clause the candidate's bone count is
    // caller-supplied work that nothing charges for.
    //
    // Measured before the clause existed, on exactly this shape: the
    // source's charge is
    //
    //   per-sample cost = 2 sides * 2 bones                       =    4
    //                   + (2 sides + 1 skin residual) * 1 slot    =    3
    //                   + 2 sides * 1 skinned vertex              =    2
    //                                                                ----
    //                                                                   9
    //   work            = 4_000 keys * 9 = 36_000
    //   budget          =                  400_000_000
    //
    // — 0.009% of the budget — while proof posed a 60_002-bone candidate
    // skeleton 4_000 times and took `3.71s` in release, more than twice
    // the wall time of a vertex-dominated document the budget scores at
    // 100%. The refusal below is raised before any of that work starts.
    let doc = budget_document(4_000, 1);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let affected = plan.affected_set();
    let affected_skin_instances = affected_skin_instance_indices(&doc, &affected);
    assert_eq!(per_sample_work_units(&doc, &affected_skin_instances), 9);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    let mut padded = candidate.document().clone();
    let root = padded.skeleton.bones[0].clone();
    padded
        .skeleton
        .bones
        .extend(std::iter::repeat_n(root, 60_000));
    assert_eq!(padded.skeleton.bones.len(), 60_002);
    let padded = ScaleCandidate { document: padded };
    assert_eq!(
        prove_scale(&doc, &padded, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "bone_count_mismatch",
        }
    );
    // The unpadded candidate still proves, so the rejection is the bone
    // count and not something the padding happened to break.
    prove_scale(&doc, &candidate, &plan).unwrap();
}

// --- Invalid declared rest/bind factor ---------------------------------

#[test]
fn a_non_positive_or_non_finite_expected_factor_is_invalid_not_a_factor_mismatch() {
    // The unit rig's observed common factor is one, so a request that
    // slipped past this guard would come back as `FactorMismatch` — a
    // materially different claim ("your rig is not what you declared")
    // from "your declared factor is not a factor".
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let capability = complete_capability();
    for factor in [0.0, -1.0, f64::NAN] {
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: factor,
            },
            document: &doc,
            capability: &capability,
        };
        match plan_scale(&request).unwrap_err() {
            ScaleError::InvalidExpectedFactor { factor: rejected } => {
                assert_eq!(rejected.is_nan(), factor.is_nan(), "{factor}");
                if !factor.is_nan() {
                    assert_eq!(rejected, factor);
                }
            }
            other => panic!("expected InvalidExpectedFactor for {factor}, got {other:?}"),
        }
    }
}

// --- Non-identity inverse binds ----------------------------------------

/// `4 * Rz(pi/2)` with a non-zero translation column: a linear part that
/// is neither identity nor a pure rotation, so `B' = U B U^-1` is a
/// genuinely different claim from "scale every component".
const NON_IDENTITY_BIND: Mat4 = Mat4::from_cols(
    Vec4::new(0.0, 4.0, 0.0, 0.0),
    Vec4::new(-4.0, 0.0, 0.0, 0.0),
    Vec4::new(0.0, 0.0, 4.0, 0.0),
    Vec4::new(5.0, -6.0, 7.0, 1.0),
);

#[test]
fn whole_document_conversion_conjugates_a_non_identity_bind_and_the_bone_convenience_value() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, NON_IDENTITY_BIND);
    // Every other fixture leaves this `None`, so the bone-level rewrite
    // branch never executes.
    doc.skeleton.bones[1].inverse_bind = Some(NON_IDENTITY_BIND);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // `U B U^-1` for a uniform `U = scale(0.01)`: the translation column
    // is multiplied by `q`, the dimensionless linear part is untouched.
    for (label, converted) in [
        (
            "instance",
            candidate.document().assets.instances[0].skin_ibms[0],
        ),
        (
            "bone",
            candidate.document().skeleton.bones[1]
                .inverse_bind
                .expect("bone bind is retained"),
        ),
    ] {
        assert_eq!(converted.x_axis, Vec4::new(0.0, 4.0, 0.0, 0.0), "{label}");
        assert_eq!(converted.y_axis, Vec4::new(-4.0, 0.0, 0.0, 0.0), "{label}");
        assert_eq!(converted.z_axis, Vec4::new(0.0, 0.0, 4.0, 0.0), "{label}");
        assert!(
            converted
                .w_axis
                .abs_diff_eq(Vec4::new(0.05, -0.06, 0.07, 1.0), 1e-7),
            "{label}: {:?}",
            converted.w_axis
        );
    }
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert!(proof.skin_matrix.max() < 1e-4);
}

#[test]
fn rest_bind_rewrites_the_bone_convenience_inverse_bind_it_falls_back_to() {
    // `skin_ibms` is empty, so the documented fallback chain resolves the
    // joint's bind through `Bone::inverse_bind` — the value every other
    // fixture leaves `None`.
    let nodes = vec![
        RigNode {
            parent: None,
            source_node_index: 0,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        },
        rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
    ];
    let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    doc.assets.instances[0].skin_ibms.clear();
    // `W1(rest) = [0.01 I | (0, 1, 0)]`, so its exact inverse bind is
    // `[100 I | (0, -100, 0)]`.
    doc.skeleton.bones[1].inverse_bind = Some(Mat4::from_scale_rotation_translation(
        Vec3::splat(100.0),
        Quat::IDENTITY,
        Vec3::new(0.0, -100.0, 0.0),
    ));
    let capability = complete_capability();
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: &doc,
        capability: &capability,
    })
    .unwrap();
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    // `B' = C^-1 * B = scale(0.01) * [100 I | (0, -100, 0)]`.
    let expected = Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0));
    let bone_bind = candidate.document().skeleton.bones[1]
        .inverse_bind
        .expect("bone bind is retained");
    assert!(bone_bind.abs_diff_eq(expected, 1e-5), "{bone_bind:?}");
    let materialized = &candidate.document().assets.instances[0].skin_ibms;
    assert_eq!(materialized.len(), 1);
    assert!(materialized[0].abs_diff_eq(expected, 1e-5));
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert!(proof.skin_matrix.max() < 1e-4);
}

// --- Skins outside the affected closure (issue #296) -------------------

/// Append a second, entirely unrelated skinned instance to `doc`: a new
/// root bone that is the only joint of a new skin, drawing the mesh the
/// document already has.
///
/// The new bone is a *root*, so no rest/bind closure rooted at bone 0 can
/// reach it: the closure is the scaled root, the selected skin's joints,
/// the ancestor paths between them, and their descendants, and this bone
/// is none of those. `check_skin_and_bounds` therefore skips its instance
/// outright, which is exactly the gap #296 names.
///
/// `bind` is what the instance *stores* per slot: `Some` writes a
/// one-element `skin_ibms`, `None` leaves it empty so the slot falls back
/// to the bone convenience value — which `Bone::inverse_bind` also leaves
/// unset, making the skin evidence-free on this side.
fn push_unrelated_skin(doc: &mut Document, bind: Option<Mat4>) -> BoneId {
    let bone = doc.skeleton.bones.len();
    let source_node_index = doc
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| node.source_node_index)
        .max()
        .expect("the base document projects at least one node")
        + 1;
    doc.skeleton.bones.push(Bone {
        name: "unrelated".into(),
        parent: None,
        rest: Transform {
            translation: Vec3::new(5.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
        inverse_bind: None,
    });
    doc.assets.source_skeleton.nodes.push(SourceNodeAsset {
        source_node_index,
        name: None,
        parent_source_node_index: None,
        scene_root_indices: vec![0],
        local_rest: SourceNodeLocalRest::Trs {
            translation: Vec3::new(5.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
        bone: Some(bone),
    });
    let source_skin_index = doc
        .assets
        .source_skeleton
        .skins
        .iter()
        .map(|skin| skin.source_skin_index)
        .max()
        .expect("the base document projects at least one skin")
        + 1;
    doc.assets.source_skeleton.skins.push(SourceSkinAsset {
        source_skin_index,
        name: None,
        skeleton_root_source_node_index: None,
        joint_source_node_indices: vec![source_node_index],
        inverse_bind_accessor: SourceInverseBindAccessor::default(),
        attachments: Vec::new(),
    });
    doc.assets.instances.push(MeshInstance {
        source_node_index,
        node: bone,
        mesh: 0,
        skin_joints: vec![bone],
        skin_ibms: bind.into_iter().collect(),
    });
    bone
}

/// `compensated_document` plus that unrelated skin — a rest/bind source
/// whose closure is `{0, 1, 2}` and whose second instance is bound to
/// bone 3 alone.
fn compensated_document_with_unrelated_skin(bind: Option<Mat4>) -> Document {
    let mut doc = compensated_document();
    assert_eq!(push_unrelated_skin(&mut doc, bind), 3);
    doc
}

/// Attach the unrelated instance to source-skin evidence of `status`.
fn attach_unrelated_source_skin(doc: &mut Document, status: SourceInverseBindAccessorStatus) {
    assert_eq!(
        doc.assets.source_skeleton.coverage,
        SourceSkeletonCoverage::Complete
    );
    let instance = &doc.assets.instances[1];
    let source_node_index = instance.source_node_index;
    let source_mesh_index = Some(doc.assets.meshes[instance.mesh].source_mesh_index);
    let skin = doc
        .assets
        .source_skeleton
        .skins
        .last_mut()
        .expect("the unrelated skin has source evidence");
    assert_eq!(
        skin.inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Absent
    );
    skin.inverse_bind_accessor.status = status;
    skin.attachments = vec![SourceSkinAttachment {
        source_node_index,
        source_mesh_index,
    }];
}

fn compensated_rest_bind_plan(doc: &Document, capability: &ScaleCapabilityFacts) -> ScalePlan {
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: doc,
        capability,
    })
    .unwrap()
}

#[test]
fn a_rewritten_unaffected_skins_inverse_binds_are_named_by_their_own_obligation() {
    // The #296 defect, exactly: bone 3 is in no closure, so the skin and
    // bounds obligations skip its instance entirely and a candidate that
    // rewrote its binds proved `Ok`.
    let doc = compensated_document_with_unrelated_skin(Some(Mat4::from_scale(Vec3::splat(2.0))));
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);

    // The honest candidate leaves the unrelated skin alone, and the
    // obligation proves that rather than skipping it.
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    assert_eq!(
        candidate.document().assets.instances[1].skin_ibms,
        doc.assets.instances[1].skin_ibms
    );
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.unaffected_inverse_bind.max(), 0.0);

    // Doctored: `scale(2)` becomes `scale(3)`. Both are diagonal, so the
    // largest component difference — which is what `matrix_residual`
    // reports — is `abs(2 - 3) = 1` on each of the three axis diagonals,
    // against a `1e-6 + 1e-5 * 3 = 3.1e-5` scalar tolerance.
    let mut broken = candidate.document().clone();
    broken.assets.instances[1].skin_ibms[0] = Mat4::from_scale(Vec3::splat(3.0));
    let error = prove_scale(&doc, &ScaleCandidate { document: broken }, &plan).unwrap_err();
    let ScaleError::ProofResidualExceeded {
        kind: ProofResidualKind::UnaffectedInverseBind,
        observed,
        ..
    } = error
    else {
        panic!("expected an unaffected-inverse-bind residual, got {error:?}");
    };
    assert_eq!(observed, 1.0);
}

#[test]
fn a_successful_unaffected_bind_proof_reports_the_residual_maximum_it_observed() {
    // Every other assertion on `unaffected_inverse_bind.max()` in this
    // module reads `0.0`, which is the field's initialized value: none of
    // them can tell a proof that measured and folded a residual from one
    // that never wrote the field at all. #284 will serialize this number,
    // so a permanently-zero field is a permanently-wrong record.
    //
    // The candidate's bind is perturbed *inside* the tolerance this
    // comparison derives for itself, so the proof succeeds and the
    // reported maximum is the perturbation:
    //
    //   source bind    diag(2, 2, 2) with translation column (64, 0, 0)
    //   candidate bind diag(2 + 2^-18, 2 + 2^-17, 2), same translation
    //
    // Both perturbations are exact in binary32 (`2.0` is `2^1`, whose ulp
    // is `2^-22`), `matrix_residual` is an L-infinity fold over the
    // sixteen components, and the larger of the two is `2^-17`. The
    // tolerance is `1e-6 + 1e-5 * max(64, 64) = 6.41e-4`, roughly `84x`
    // the residual, so this is a pass with headroom and not a boundary
    // case. The smaller perturbation is there so the reported number is a
    // genuine maximum rather than the only candidate for one.
    //
    // The translation column is what makes this the rest/bind fixture
    // that separates the two expectations this obligation can state. A
    // bind with a zero translation column is a fixed point of
    // `scale_translation_only`, so on one the whole-document conversion
    // expectation and the rest/bind "unchanged" expectation are the same
    // matrix and the `is_whole_document` branch is unobservable. At `64`
    // the conversion expectation would be `0.64` and this proof would
    // fail instead of reporting a residual.
    let bind = |scale: Vec3| {
        let mut bind = Mat4::from_scale(scale);
        bind.w_axis.x = 64.0;
        bind
    };
    let doc = compensated_document_with_unrelated_skin(Some(bind(Vec3::splat(2.0))));
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    let mut nudged = candidate.document().clone();
    nudged.assets.instances[1].skin_ibms[0] =
        bind(Vec3::new(2.0 + 2f32.powi(-18), 2.0 + 2f32.powi(-17), 2.0));
    let proof = prove_scale(&doc, &ScaleCandidate { document: nudged }, &plan).unwrap();
    assert_eq!(proof.unaffected_inverse_bind.max(), 2f64.powi(-17));
}

#[test]
fn a_partially_affected_skin_stays_with_the_skin_obligation_that_owns_it() {
    // The skip predicate is `any`, not `all`, and the difference is not
    // cosmetic. An instance with *some* joint in the closure is the skin
    // obligation's, which checks `W * B` on both sides; holding its binds
    // to "unchanged" as well would refuse the honest candidate this very
    // module builds, because `build_rest_bind` rewrites exactly the slots
    // whose joint is affected. That is the fail-closed regression class
    // #296 was deferred from #288 to avoid, re-introduced from the other
    // end.
    //
    // Slot 0 is bone 3 (outside every closure), slot 1 is bone 1 (inside
    // it), so this instance is partially affected and no other fixture
    // here is.
    let unaffected_bind = Mat4::from_scale(Vec3::splat(2.0));
    let mut doc = compensated_document_with_unrelated_skin(Some(unaffected_bind));
    let affected_bind = doc.assets.instances[0].skin_ibms[0];
    doc.assets.instances[1].skin_joints = vec![3, 1];
    doc.assets.instances[1].skin_ibms = vec![unaffected_bind, affected_bind];

    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let rebased = &candidate.document().assets.instances[1].skin_ibms;
    // The unaffected slot came through byte-identical; the affected one
    // was conjugated by `scale(s)` and did not.
    assert_eq!(rebased[0], unaffected_bind);
    assert_ne!(rebased[1], affected_bind);

    // The builder's own output proves. Under an `all` predicate this
    // instance is no longer skipped, slot 1 is compared against the
    // source bind it was deliberately rebased away from, and this call
    // returns `ProofResidualExceeded { UnaffectedInverseBind }`.
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.unaffected_inverse_bind.max(), 0.0);
}

#[test]
fn a_slot_carrying_both_an_array_and_a_bone_bind_is_compared_through_the_array() {
    // The precedence `instance_bind` documents — the per-instance array
    // first, then the bone convenience value, then a licensed identity
    // default — is normative in §D.6. No other fixture gives one slot *both*
    // stored forms, so reversing the first two is invisible: the array-only
    // fixtures resolve through the array either way, and the bone-only fixture
    // through the bone.
    //
    // Here the unrelated slot carries an array bind of `scale(2)` and a
    // bone bind of `scale(4)`, and the two directions are separated by
    // doctoring exactly one of them in the candidate.
    let array_bind = Mat4::from_scale(Vec3::splat(2.0));
    let bone_bind = Mat4::from_scale(Vec3::splat(4.0));
    let mut doc = compensated_document_with_unrelated_skin(Some(array_bind));
    doc.skeleton.bones[3].inverse_bind = Some(bone_bind);
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // The array is the authority, so a rewritten array is caught even
    // though the shadowed bone bind still matches. `scale(2)` against
    // `scale(3)` differs by `1` on each of three diagonal components,
    // against a `1e-6 + 1e-5 * 3 = 3.1e-5` tolerance.
    let mut rewritten_array = candidate.document().clone();
    rewritten_array.assets.instances[1].skin_ibms[0] = Mat4::from_scale(Vec3::splat(3.0));
    let error = prove_scale(
        &doc,
        &ScaleCandidate {
            document: rewritten_array,
        },
        &plan,
    )
    .unwrap_err();
    let ScaleError::ProofResidualExceeded {
        kind: ProofResidualKind::UnaffectedInverseBind,
        observed,
        ..
    } = error
    else {
        panic!("expected an unaffected-inverse-bind residual, got {error:?}");
    };
    assert_eq!(observed, 1.0);

    // And the converse, which is the half a one-directional test misses:
    // a bone bind shadowed by a non-empty array is not authority for this
    // slot, so rewriting it changes nothing this obligation claims. Under
    // the reversed precedence this call fails instead.
    let mut rewritten_bone = candidate.document().clone();
    rewritten_bone.skeleton.bones[3].inverse_bind = Some(Mat4::from_scale(Vec3::splat(5.0)));
    let proof = prove_scale(
        &doc,
        &ScaleCandidate {
            document: rewritten_bone,
        },
        &plan,
    )
    .unwrap();
    assert_eq!(proof.unaffected_inverse_bind.max(), 0.0);
}

#[test]
fn an_unaffected_skin_resolved_through_its_bones_is_proved_the_same_way() {
    // The instance stores no array, so both sides resolve the slot
    // through `Bone::inverse_bind` — the module's documented fallback.
    // That evidence is compared too: "the instance stored nothing" is not
    // the same as "there is nothing to compare".
    let mut doc = compensated_document_with_unrelated_skin(None);
    doc.skeleton.bones[3].inverse_bind = Some(Mat4::from_scale(Vec3::splat(2.0)));
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.unaffected_inverse_bind.max(), 0.0);

    // Same arithmetic as the instance-array case above: a diagonal
    // `scale(2)` doctored to `scale(3)` differs by `1` per axis.
    let mut broken = candidate.document().clone();
    broken.skeleton.bones[3].inverse_bind = Some(Mat4::from_scale(Vec3::splat(3.0)));
    let error = prove_scale(&doc, &ScaleCandidate { document: broken }, &plan).unwrap_err();
    let ScaleError::ProofResidualExceeded {
        kind: ProofResidualKind::UnaffectedInverseBind,
        observed,
        ..
    } = error
    else {
        panic!("expected an unaffected-inverse-bind residual, got {error:?}");
    };
    assert_eq!(observed, 1.0);
}

#[test]
fn an_unaffected_skin_with_no_bind_evidence_on_either_side_still_proves() {
    // Neither side stores a bind, and the complete attached source skin says
    // its accessor is unreadable rather than absent, so it cannot license the
    // format-defined identity default. Nothing is claimed about this genuinely
    // evidence-free slot, and nothing needs to be — but the document proves.
    let mut doc = compensated_document_with_unrelated_skin(None);
    attach_unrelated_source_skin(&mut doc, SourceInverseBindAccessorStatus::Unreadable);
    assert!(doc.assets.instances[1].skin_ibms.is_empty());
    assert!(doc.skeleton.bones[3].inverse_bind.is_none());
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    // The count, not the residual, is what this test turns on. A slot
    // neither side records is out of the proof's scope, and DESIGN.md
    // Appendix D §D.6 requires that to read as an absence — a count of
    // zero. A zero *residual* is also what comparing two identities
    // would publish, so it cannot tell the two apart on its own.
    assert_eq!(proof.unaffected_inverse_bind.comparisons(), 0);
    assert_eq!(proof.unaffected_inverse_bind.max(), 0.0);
}

fn two_defaulted_identity_slots_document() -> Document {
    // Both sides omit every stored bind, but the complete attached source
    // skin licenses the format-defined identity default for both slots.
    let mut doc = compensated_document_with_unrelated_skin(None);
    attach_unrelated_source_skin(&mut doc, SourceInverseBindAccessorStatus::Absent);

    let second_bone = doc.skeleton.bones.len();
    let second_source_node_index = doc
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| node.source_node_index)
        .max()
        .expect("the document projects at least one node")
        + 1;
    let second_rest = Transform {
        translation: Vec3::new(7.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    doc.skeleton.bones.push(Bone {
        name: "unrelated-second".into(),
        parent: None,
        rest: second_rest,
        inverse_bind: None,
    });
    doc.assets.source_skeleton.nodes.push(SourceNodeAsset {
        source_node_index: second_source_node_index,
        name: None,
        parent_source_node_index: None,
        scene_root_indices: vec![0],
        local_rest: SourceNodeLocalRest::Trs {
            translation: second_rest.translation,
            rotation: second_rest.rotation,
            scale: second_rest.scale,
        },
        bone: Some(second_bone),
    });
    doc.assets
        .source_skeleton
        .skins
        .last_mut()
        .expect("the unrelated skin has source evidence")
        .joint_source_node_indices
        .push(second_source_node_index);
    doc.assets.instances[1].skin_joints.push(second_bone);

    assert!(doc.assets.instances[1].skin_ibms.is_empty());
    assert!(
        doc.skeleton.bones[3..]
            .iter()
            .all(|bone| bone.inverse_bind.is_none())
    );
    doc
}

#[test]
fn two_defaulted_identity_slots_are_each_compared_as_effective_binds() {
    // Keeping two slots makes the comparison count pin the "every slot"
    // obligation as well as the both-defaulted fallback.
    let doc = two_defaulted_identity_slots_document();
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    assert!(
        candidate.document().assets.instances[1]
            .skin_ibms
            .is_empty()
    );
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    assert_eq!(proof.unaffected_inverse_bind.comparisons(), 2);
    assert_eq!(proof.unaffected_inverse_bind.max(), 0.0);
}

fn assert_unaffected_bind_rewrite(error: ScaleError) {
    let ScaleError::ProofResidualExceeded {
        kind: ProofResidualKind::UnaffectedInverseBind,
        observed,
        ..
    } = error
    else {
        panic!("expected an unaffected-inverse-bind residual, got {error:?}");
    };
    assert_eq!(observed, 1.0);
}

#[test]
fn materializing_a_different_bind_in_defaulted_slot_zero_is_refused() {
    // Slot 1 remains explicit identity, so an implementation that compares
    // only slot 1 (or compares it twice) cannot observe this rewrite.
    let doc = two_defaulted_identity_slots_document();
    let plan = compensated_rest_bind_plan(&doc, &complete_capability());
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut rewritten = candidate.document().clone();
    rewritten.assets.instances[1].skin_ibms =
        vec![Mat4::from_scale(Vec3::splat(2.0)), Mat4::IDENTITY];

    let error = prove_scale(
        &doc,
        &ScaleCandidate {
            document: rewritten,
        },
        &plan,
    )
    .unwrap_err();
    assert_unaffected_bind_rewrite(error);
}

#[test]
fn materializing_a_different_bind_in_defaulted_slot_one_is_refused() {
    // Slot 0 remains explicit identity, so an implementation that compares
    // only slot 0 (or compares it twice) cannot observe this rewrite.
    let doc = two_defaulted_identity_slots_document();
    let plan = compensated_rest_bind_plan(&doc, &complete_capability());
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut rewritten = candidate.document().clone();
    rewritten.assets.instances[1].skin_ibms =
        vec![Mat4::IDENTITY, Mat4::from_scale(Vec3::splat(2.0))];

    let error = prove_scale(
        &doc,
        &ScaleCandidate {
            document: rewritten,
        },
        &plan,
    )
    .unwrap_err();
    assert_unaffected_bind_rewrite(error);
}

#[test]
fn explicit_and_defaulted_identity_are_the_same_unaffected_bind_in_both_directions() {
    let capability = complete_capability();

    // Defaulted source, explicit candidate: both resolve to identity and one
    // effective comparison proves, despite the representation change.
    let mut defaulted = compensated_document_with_unrelated_skin(None);
    attach_unrelated_source_skin(&mut defaulted, SourceInverseBindAccessorStatus::Absent);
    let plan = compensated_rest_bind_plan(&defaulted, &capability);
    let candidate = build_scale_candidate(&defaulted, &plan).unwrap();
    let mut explicit = candidate.document().clone();
    explicit.assets.instances[1].skin_ibms = vec![Mat4::IDENTITY];
    let proof = prove_scale(&defaulted, &ScaleCandidate { document: explicit }, &plan).unwrap();
    assert_eq!(proof.unaffected_inverse_bind.comparisons(), 1);
    assert_eq!(proof.unaffected_inverse_bind.max(), 0.0);

    // The same representation change in reverse also proves.
    let mut explicit = compensated_document_with_unrelated_skin(Some(Mat4::IDENTITY));
    attach_unrelated_source_skin(&mut explicit, SourceInverseBindAccessorStatus::Absent);
    let plan = compensated_rest_bind_plan(&explicit, &capability);
    let candidate = build_scale_candidate(&explicit, &plan).unwrap();
    let mut defaulted = candidate.document().clone();
    defaulted.assets.instances[1].skin_ibms.clear();
    let proof = prove_scale(
        &explicit,
        &ScaleCandidate {
            document: defaulted,
        },
        &plan,
    )
    .unwrap();
    assert_eq!(proof.unaffected_inverse_bind.comparisons(), 1);
    assert_eq!(proof.unaffected_inverse_bind.max(), 0.0);
}

#[test]
fn a_materialized_nonidentity_default_is_a_rewritten_unaffected_bind() {
    // Materializing a different matrix is a rewritten unaffected bind, refused
    // by the value residual rather than representation alone.
    let capability = complete_capability();
    let mut defaulted = compensated_document_with_unrelated_skin(None);
    attach_unrelated_source_skin(&mut defaulted, SourceInverseBindAccessorStatus::Absent);
    let plan = compensated_rest_bind_plan(&defaulted, &capability);
    let candidate = build_scale_candidate(&defaulted, &plan).unwrap();
    let mut rewritten = candidate.document().clone();
    rewritten.assets.instances[1].skin_ibms = vec![Mat4::from_scale(Vec3::splat(2.0))];
    let error = prove_scale(
        &defaulted,
        &ScaleCandidate {
            document: rewritten,
        },
        &plan,
    )
    .unwrap_err();
    let ScaleError::ProofResidualExceeded {
        kind: ProofResidualKind::UnaffectedInverseBind,
        observed,
        ..
    } = error
    else {
        panic!("expected an unaffected-inverse-bind residual, got {error:?}");
    };
    assert_eq!(observed, 1.0);
}

#[test]
fn an_unaffected_skin_whose_bind_evidence_appears_on_only_one_side_is_missing_not_proven() {
    // Between "unchanged, and I can prove it" and "no evidence either
    // way" sits a third case: one side records a bind and the other does
    // not. That is a rewritten skin — the candidate either dropped an
    // array, silently changing which bind the slot resolves to, or
    // materialized one the source never had — and it is reported as
    // missing evidence rather than passed for want of a comparison. Neither
    // fixture attaches an absent-accessor source skin to the unrelated
    // instance, so no format-defined identity is licensed here.
    let capability = complete_capability();

    let doc = compensated_document_with_unrelated_skin(Some(Mat4::from_scale(Vec3::splat(2.0))));
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut dropped = candidate.document().clone();
    dropped.assets.instances[1].skin_ibms.clear();
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate { document: dropped }, &plan).unwrap_err(),
        ScaleError::MissingProofEvidence {
            kind: ProofResidualKind::UnaffectedInverseBind,
            detail: "candidate_slot_bind_missing",
        }
    );

    let bare = compensated_document_with_unrelated_skin(None);
    let bare_plan = compensated_rest_bind_plan(&bare, &capability);
    let bare_candidate = build_scale_candidate(&bare, &bare_plan).unwrap();
    let mut invented = bare_candidate.document().clone();
    invented.assets.instances[1].skin_ibms = vec![Mat4::from_scale(Vec3::splat(2.0))];
    assert_eq!(
        prove_scale(&bare, &ScaleCandidate { document: invented }, &bare_plan).unwrap_err(),
        ScaleError::MissingProofEvidence {
            kind: ProofResidualKind::UnaffectedInverseBind,
            detail: "source_slot_bind_missing",
        }
    );
}

#[test]
fn a_whole_document_plan_cannot_omit_a_bone_added_after_planning() {
    // A stale plan over bones 0 and 1 once proved a wider source while
    // walking only those two ids. Restoring added bone 2's candidate rest
    // translation from the correct `0.05` to its source value `5.0`
    // therefore returned `Ok`: whole-document disabled the complement,
    // and no other obligation reached that bone. Re-deriving the plan
    // inventory against the supplied source must reject before either
    // build or proof can silently omit it.
    let capability = complete_capability();
    let planned = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
        document: &planned,
        capability: &capability,
    })
    .unwrap();
    assert_eq!(plan.affected_nodes(), &[0, 1]);

    let mut wider = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let added = push_unrelated_skin(
        &mut wider,
        Some(Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0))),
    );
    assert_eq!(added, 2);
    let expected = ScaleError::PlanDocumentMismatch {
        reason: "affected_nodes_mismatch",
    };
    assert_eq!(build_scale_candidate(&wider, &plan).unwrap_err(), expected);

    // Bypass the public builder only to reproduce the old proof exploit:
    // start from the correct whole-document rewrite, then restore the new
    // bone's normalized/raw rest and its instance bind so every payload
    // the stale plan omits is consistently left unconverted.
    let wider_plan = whole_document_plan(&wider, &complete_capability());
    let mut omitted = build_whole_document(&wider, &wider_plan).unwrap();
    omitted.skeleton.bones[added].rest = wider.skeleton.bones[added].rest;
    let source_local_rest = wider
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(added))
        .unwrap()
        .local_rest
        .clone();
    omitted
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(added))
        .unwrap()
        .local_rest = source_local_rest;
    omitted.assets.instances[1].skin_ibms[0] = wider.assets.instances[1].skin_ibms[0];
    assert_eq!(
        prove_scale(&wider, &ScaleCandidate { document: omitted }, &plan).unwrap_err(),
        expected
    );
}

// --- Stable reason strings ---------------------------------------------

fn rest_bind_reject_reason(document: &Document) -> ScaleError {
    let capability = complete_capability();
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
        document,
        capability: &capability,
    })
    .unwrap_err()
}

/// The reason [`rest_bind_affected_closure`] names for `document`, reached
/// by calling the helper directly rather than through [`plan_scale`].
///
/// Its *malformed-projection* guards — a cyclic or unbounded ancestor
/// walk, a dangling `parent_source_node_index` — are reached here from a
/// *projected* node's chain, and from that direction they are shadowed:
/// rest/bind planning requires [`SourceSkeletonCoverage::Complete`], and
/// under that coverage [`crate::model::validate_document_shape`] has already
/// established that every projected node's ancestor chain terminates,
/// stays inside the table, and agrees with a skeleton that is acyclic and
/// orders every parent before its child. Each test below pins both halves:
/// the guard's own reason through this helper, and the public refusal that
/// shadows it.
///
/// They are *not* dead through the public API, and must not be described
/// as such. Validation quantifies over projected nodes only —
/// deliberately, since a node that never became a bone cannot be displaced
/// — while this closure walks the ancestor chain of every node named in
/// [`SourceSkinAsset::joint_source_node_indices`], projected or not. An
/// unprojected skin joint therefore reaches both guards through
/// [`plan_scale`], which
/// `an_unprojected_skin_joint_with_a_dangling_parent_is_refused_by_planning`
/// and `an_unprojected_skin_joint_on_a_cyclic_chain_is_refused_by_planning`
/// pin. That ordering is not incidental: the closure runs before the
/// domain's `SourceNodeNotNormalized` check, so the malformed chain is
/// what the caller is told about.
///
/// The guards whose cause is *not* a parent-chain fact — a skin joint
/// with no projection, a descendant joint owned by another skin,
/// unskinned geometry inside the closure — stay reachable through
/// [`rest_bind_reject_reason`].
fn closure_reject_reason(document: &Document, source_root_node_index: usize) -> ScaleError {
    let by_source_index = source_node_index_map(document);
    let skin = resolve_rest_bind_skin(document, 0).expect("the fixture declares source skin 0");
    rest_bind_affected_closure(document, &by_source_index, skin, source_root_node_index)
        .expect_err("the fixture's projection is malformed")
}

/// The reason [`source_world_matrix`] names for `start` in `document`,
/// reached by calling the helper directly.
///
/// These two guards — `cyclic_source_parent_chain` and
/// `missing_source_node` — *are* unreachable through [`plan_scale`], and
/// unlike the closure's the claim survives the projected/unprojected
/// split: [`plan_rest_bind`] resolves every domain node's bone, refusing
/// an unprojected one with [`ScaleError::SourceNodeNotNormalized`], before
/// composing a single world matrix. So this walk only ever starts at a
/// projected node, whose whole chain
/// [`crate::model::validate_document_shape`] has already accepted.
fn source_world_reject_reason(document: &Document, start: usize) -> ScaleError {
    let by_source_index = source_node_index_map(document);
    let mut cache = BTreeMap::new();
    source_world_matrix(start, &by_source_index, &BTreeSet::new(), &mut cache)
        .expect_err("the fixture's projection is malformed")
}

#[test]
fn a_skin_joint_with_no_source_node_projection_names_its_own_closure_reason() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.assets.source_skeleton.skins[0]
        .joint_source_node_indices
        .push(99);
    assert_eq!(
        rest_bind_reject_reason(&doc),
        ScaleError::IncompleteClosure {
            reason: "skin_joint_source_node_missing"
        }
    );
}

#[test]
fn a_valid_unprojected_selected_joint_is_refused_as_not_normalized() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let mut joint = SourceNodeAsset::new(5, SourceNodeLocalRest::Matrix(Mat4::IDENTITY));
    joint.parent_source_node_index = Some(0);
    doc.assets.source_skeleton.nodes.push(joint);
    doc.assets.source_skeleton.skins[0]
        .joint_source_node_indices
        .push(5);
    assert_eq!(
        rest_bind_reject_reason(&doc),
        ScaleError::SourceNodeNotNormalized {
            source_node_index: 5
        }
    );
}

#[test]
fn an_unprojected_selected_root_is_refused_as_not_normalized() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.assets.source_skeleton.nodes[0].bone = None;
    // Keep the remaining projected row's normalized parent consistent:
    // its raw nearest *projected* ancestor is now absent as well.
    doc.skeleton.bones[1].parent = None;
    let capability = complete_capability();
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap_err(),
        ScaleError::SourceNodeNotNormalized {
            source_node_index: 0
        }
    );
}

/// `unit_rig` plus one unprojected source node listed as a skin joint —
/// the shape that keeps the closure's malformed-projection guards live
/// through [`plan_scale`], because chain validation covers projected nodes
/// only while the closure walks every declared joint.
fn unprojected_skin_joint_document(chain: &[(usize, Option<usize>)]) -> Document {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    for &(source_node_index, parent) in chain {
        let mut evidence = SourceNodeAsset::new(
            source_node_index,
            SourceNodeLocalRest::Matrix(Mat4::IDENTITY),
        );
        evidence.parent_source_node_index = parent;
        assert_eq!(evidence.bone, None);
        doc.assets.source_skeleton.nodes.push(evidence);
    }
    doc.assets.source_skeleton.skins[0]
        .joint_source_node_indices
        .push(chain[0].0);
    doc
}

#[test]
fn an_unprojected_skin_joint_with_a_dangling_parent_is_refused_by_planning() {
    // Node 5 names no bone, so the shared projection check skips it
    // — and the skin names it a joint, so the closure walks its chain and
    // finds node 99 absent. The guard is not shadowed here; it is the
    // refusal.
    let doc = unprojected_skin_joint_document(&[(5, Some(99))]);
    assert_eq!(
        rest_bind_reject_reason(&doc),
        ScaleError::IncompleteClosure {
            reason: "dangling_source_parent_node_index"
        }
    );
}

#[test]
fn an_unprojected_skin_joint_on_a_cyclic_chain_is_refused_by_planning() {
    // The same gap, reached by the other guard: nodes 5 and 6 name each
    // other and neither names a bone.
    let doc = unprojected_skin_joint_document(&[(5, Some(6)), (6, Some(5))]);
    assert_eq!(
        rest_bind_reject_reason(&doc),
        ScaleError::IncompleteClosure {
            reason: "cyclic_or_unbounded_source_parent_chain"
        }
    );
}

#[test]
fn a_descendant_claimed_as_a_joint_by_another_skin_names_its_own_closure_reason() {
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
    ];
    let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    doc.assets.source_skeleton.skins.push(SourceSkinAsset {
        source_skin_index: 1,
        name: None,
        skeleton_root_source_node_index: None,
        joint_source_node_indices: vec![2],
        inverse_bind_accessor: SourceInverseBindAccessor::default(),
        attachments: Vec::new(),
    });
    assert_eq!(
        rest_bind_reject_reason(&doc),
        ScaleError::IncompleteClosure {
            reason: "descendant_joint_of_another_skin"
        }
    );
}

#[test]
fn a_joint_ancestor_chain_that_never_reaches_the_root_names_its_own_closure_reason() {
    // Source nodes 1 and 2 name each other as parent, so walking joint
    // 2's ancestor chain toward the declared root never terminates.
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
    ];
    let mut doc = rig_document(&nodes, &[2], 0, Mat4::IDENTITY);
    doc.assets.source_skeleton.nodes[1].parent_source_node_index = Some(2);
    assert_eq!(
        closure_reject_reason(&doc, 0),
        ScaleError::IncompleteClosure {
            reason: "cyclic_or_unbounded_source_parent_chain"
        }
    );
    // And the public path never gets there: node 1's projected parent is
    // node 2 while `Skeleton::parent` says bone 0.
    assert_eq!(
        rest_bind_reject_reason(&doc),
        ScaleError::InvalidDocumentShape(DocumentShapeError::SourceProjection {
            source_node_index: 1,
            violation: SourceProjectionViolation::NearestProjectedParentMismatch,
        })
    );
}

#[test]
fn a_cyclic_rest_world_parent_chain_names_its_own_closure_reason() {
    // The closure itself completes — joint 1 reaches the declared root
    // in one hop — but composing the root's own rest-world matrix walks
    // *above* the closure and finds the root naming its own descendant
    // as parent.
    let nodes = vec![
        rig(None, 0, Vec3::ZERO),
        rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        rig(Some(0), 2, Vec3::new(0.0, 1.0, 0.0)),
    ];
    let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
    doc.assets.source_skeleton.nodes[0].parent_source_node_index = Some(2);
    assert_eq!(
        source_world_reject_reason(&doc, 0),
        ScaleError::IncompleteClosure {
            reason: "cyclic_source_parent_chain"
        }
    );
    // And the public path never gets there: node 0 is a projection child
    // of node 2 while `Skeleton::parent` says bone 0 is a root.
    assert_eq!(
        rest_bind_reject_reason(&doc),
        ScaleError::InvalidDocumentShape(DocumentShapeError::SourceProjection {
            source_node_index: 0,
            violation: SourceProjectionViolation::NearestProjectedParentMismatch,
        })
    );
}

#[test]
fn a_rest_world_ancestor_outside_the_projection_names_its_own_closure_reason() {
    // The scaled root declares an ancestor the source-node projection
    // does not carry, so its true rest-world linear part is unknowable.
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.assets.source_skeleton.nodes[0].parent_source_node_index = Some(99);
    assert_eq!(
        source_world_reject_reason(&doc, 0),
        ScaleError::IncompleteClosure {
            reason: "missing_source_node"
        }
    );
    // And the public path never gets there: an ancestor the projection
    // does not carry is an ancestor no bone can be found for.
    assert_eq!(
        rest_bind_reject_reason(&doc),
        ScaleError::InvalidDocumentShape(DocumentShapeError::SourceProjection {
            source_node_index: 0,
            violation: SourceProjectionViolation::ParentSourceNodeMissing,
        })
    );
}

/// One row of the candidate-structure table: the stable reason the
/// mismatch must be named by, and the doctoring that produces it.
type StructureMismatchCase = (&'static str, fn(&mut Document));

#[test]
fn every_candidate_structure_mismatch_names_its_own_reason() {
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    doc.clips.push(Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ONE]),
        }],
    });
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    let cases: [StructureMismatchCase; 14] = [
        // The extra bone is a parentless copy of the root, so it passes
        // `validate_scale_input` (which runs first) and reaches the
        // parity clause rather than a shape guard. This is the row the
        // sampling budget depends on: `per_sample_work_units` measures
        // only the source skeleton while `sample_time_obligations` poses
        // both, so an unrejected candidate skeleton is unbilled work.
        ("bone_count_mismatch", |d| {
            let root = d.skeleton.bones[0].clone();
            d.skeleton.bones.push(root);
        }),
        ("skeleton_topology_mismatch", |d| {
            d.skeleton.bones[1].parent = None;
            d.assets
                .source_skeleton
                .nodes
                .iter_mut()
                .find(|node| node.bone == Some(1))
                .unwrap()
                .parent_source_node_index = None;
        }),
        ("track_count_mismatch", |d| {
            d.clips[0].tracks.push(Track {
                bone: 1,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Quats(vec![Quat::IDENTITY]),
            })
        }),
        ("track_shape_mismatch", |d| {
            d.clips[0].tracks[0].interpolation = Interpolation::Step
        }),
        // The remaining track-identity clauses, each doctored on its
        // own: a track retargeted to the sibling bone, and one
        // rebranded onto the other `Vec3`-valued channel. Both leave
        // track count, times and value count untouched, so only the
        // clause under test can name the mismatch.
        ("track_shape_mismatch", |d| d.clips[0].tracks[0].bone = 0),
        ("track_shape_mismatch", |d| {
            d.clips[0].tracks[0].property = Property::Scale
        }),
        ("instance_count_mismatch", |d| {
            let extra = d.assets.instances[0].clone();
            d.assets.instances.push(extra);
        }),
        // The placement half of "unchanged mesh/material/skin identity":
        // the instance is re-parented onto the root, and re-pointed at
        // the root's source node, one field at a time. Bone 0 and source
        // node 0 both exist, so neither doctoring can be caught by a
        // range check standing in for the parity clause.
        ("instance_node_mismatch", |d| d.assets.instances[0].node = 0),
        ("instance_source_node_index_mismatch", |d| {
            d.assets.instances[0].source_node_index = 0
        }),
        // "Unchanged mesh/material/skin identity" (DESIGN.md Appendix D
        // §D.6). The extra mesh keeps `instance.mesh = 1` in range, so
        // this reaches the parity check rather than the document-shape
        // guard; the mesh-count clause is checked *after* it, so the
        // reason below is still attributable to the instance clause.
        ("instance_mesh_mismatch", |d| {
            let extra = d.assets.meshes[0].clone();
            d.assets.meshes.push(extra);
            d.assets.instances[0].mesh = 1;
        }),
        ("instance_skin_joints_mismatch", |d| {
            d.assets.instances[0].skin_joints = vec![0];
        }),
        ("mesh_count_mismatch", |d| {
            let extra = d.assets.meshes[0].clone();
            d.assets.meshes.push(extra);
        }),
        ("primitive_count_mismatch", |d| {
            let extra = d.assets.meshes[0].primitives[0].clone();
            d.assets.meshes[0].primitives.push(extra);
        }),
        ("primitive_vertex_count_mismatch", |d| {
            d.assets.meshes[0].primitives[0].positions.push(Vec3::ZERO);
            d.assets.meshes[0].primitives[0].joints.push([0, 0, 0, 0]);
            d.assets.meshes[0].primitives[0]
                .weights
                .push([1.0, 0.0, 0.0, 0.0]);
        }),
    ];
    for (expected, doctor) in cases {
        let mut broken = candidate.document().clone();
        doctor(&mut broken);
        let broken = ScaleCandidate { document: broken };
        assert_eq!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::CandidateStructureMismatch { reason: expected }
        );
    }
}

// --- Mesh-instance placement identity (issue #307) --------------------

#[test]
fn a_candidate_that_relocates_a_mesh_instance_is_refused_field_by_field() {
    // Placement is the half of "unchanged mesh/material/skin identity"
    // no residual can reach. The rig is three bones deliberately: the one
    // instance hangs off the *middle* one and names source node 1, so
    // every field can be doctored both down (to 0) and up (to 2) while
    // staying in range. An ordering comparison rather than a parity one
    // would refuse one direction and let the other through.
    let doc = rig_document(
        &[
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
        ],
        &[1],
        0,
        Mat4::IDENTITY,
    );
    assert_eq!(doc.skeleton.bones.len(), 3);
    assert_eq!(doc.assets.instances[0].node, 1);
    assert_eq!(doc.assets.instances[0].source_node_index, 1);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    // The honest candidate proves, so each refusal below is attributable
    // to its doctoring rather than to a fixture that never proved.
    prove_scale(&doc, &candidate, &plan).unwrap();

    let refuse = |doctor: &dyn Fn(&mut MeshInstance)| {
        let mut broken = candidate.document().clone();
        doctor(&mut broken.assets.instances[0]);
        prove_scale(&doc, &ScaleCandidate { document: broken }, &plan).unwrap_err()
    };

    // `node` alone: the bone the instance hangs off, which is what the
    // glTF writer attaches it to and what measurement poses it with.
    // Both directions, because either one alone is a relocation.
    for doctored in [0, 2] {
        assert_eq!(
            refuse(&|instance| instance.node = doctored),
            ScaleError::CandidateStructureMismatch {
                reason: "instance_node_mismatch"
            },
            "moving the instance onto bone {doctored} must be refused"
        );
    }
    // `source_node_index` alone: what `instance_source_skin` matches a
    // source skin's attachments against, and so what decides whether a
    // missing bind is glTF's format-defined identity default. Again both
    // directions.
    for doctored in [0, 2] {
        assert_eq!(
            refuse(&|instance| instance.source_node_index = doctored),
            ScaleError::CandidateStructureMismatch {
                reason: "instance_source_node_index_mismatch"
            },
            "re-pointing the instance at source node {doctored} must be refused"
        );
    }
    // Both together — the whole-document construction that used to
    // return `Ok(0.0)`. The `node` clause is checked first, so that is
    // the reason reported.
    assert_eq!(
        refuse(&|instance| {
            instance.node = 0;
            instance.source_node_index = 0;
        }),
        ScaleError::CandidateStructureMismatch {
            reason: "instance_node_mismatch"
        }
    );
}

#[test]
fn a_candidate_that_swaps_two_payload_identical_instances_is_refused() {
    // Two instances drawing the same mesh, bound to the same joints,
    // with the same stored binds, differing only in where they hang.
    // Swapping them leaves every payload comparison this module makes —
    // mesh positions, skin matrices, bounds, stored binds — reading
    // exactly the values it read before, so only positional identity
    // can catch it.
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let twin = MeshInstance {
        source_node_index: 0,
        node: 0,
        ..doc.assets.instances[0].clone()
    };
    doc.assets.instances.push(twin);
    let (first, second) = (&doc.assets.instances[0], &doc.assets.instances[1]);
    assert_eq!(first.mesh, second.mesh);
    assert_eq!(first.skin_joints, second.skin_joints);
    assert_eq!(first.skin_ibms, second.skin_ibms);
    assert_ne!(first.node, second.node);

    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    prove_scale(&doc, &candidate, &plan).unwrap();

    let mut swapped = candidate.document().clone();
    swapped.assets.instances.swap(0, 1);
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate { document: swapped }, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "instance_node_mismatch"
        }
    );
}

/// `compensated_document` plus an **unskinned** prop hanging off a new
/// parentless bone at world `x = 5`, outside the affected closure.
///
/// Unskinned is what makes this the sharp fixture. Skinning ignores an
/// instance's node transform — a skinned instance deforms identically
/// wherever it is attached — so only for an unskinned prop does `node`
/// alone decide where its geometry lands, given the bone it names.
fn compensated_document_with_unskinned_prop() -> (Document, BoneId) {
    let mut doc = compensated_document();
    let bone = doc.skeleton.bones.len();
    let source_node_index = doc
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| node.source_node_index)
        .max()
        .expect("the base document projects at least one node")
        + 1;
    let rest = Transform {
        translation: Vec3::new(5.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };
    doc.skeleton.bones.push(Bone {
        name: "prop".into(),
        parent: None,
        rest,
        inverse_bind: None,
    });
    doc.assets.source_skeleton.nodes.push(SourceNodeAsset {
        source_node_index,
        name: None,
        parent_source_node_index: None,
        scene_root_indices: vec![0],
        local_rest: SourceNodeLocalRest::Trs {
            translation: rest.translation,
            rotation: rest.rotation,
            scale: rest.scale,
        },
        bone: Some(bone),
    });
    doc.assets.meshes.push(MeshAsset {
        name: "prop".into(),
        source_mesh_index: 1,
        primitives: vec![Primitive {
            positions: vec![Vec3::new(1.0, 0.0, 0.0)],
            ..Primitive::default()
        }],
    });
    let mesh = doc.assets.meshes.len() - 1;
    doc.assets.instances.push(MeshInstance {
        source_node_index,
        node: bone,
        mesh,
        skin_joints: Vec::new(),
        skin_ibms: Vec::new(),
    });
    (doc, bone)
}

#[test]
fn a_rest_bind_candidate_that_relocates_an_unskinned_prop_is_refused() {
    let (doc, prop) = compensated_document_with_unskinned_prop();
    assert_eq!(prop, 3);
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    // Bone 3 is a second scene root, so it is outside the closure and
    // untouched; bone 2 is the transform-only attachment *inside* it,
    // rebased from x = 1 to x = 0.01. Moving the prop from one to the
    // other is a real relocation, from world x = 5 onto a rebased joint.
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    assert_eq!(doc.assets.instances[1].node, prop);
    assert!(doc.assets.instances[1].skin_joints.is_empty());

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();
    // Nothing the proof measures reads the prop's node, so the relocated
    // candidate below produces exactly these residuals: every one of them
    // zero, on a document whose prop has moved five units.
    assert_eq!(proof.rest_translation.max(), 0.0);
    assert_eq!(proof.rest_rotation.max(), 0.0);
    assert!(proof.rest_rotation.evaluated());
    assert_eq!(proof.mesh_position.max(), 0.0);
    assert_eq!(proof.unaffected_inverse_bind.max(), 0.0);

    let mut relocated = candidate.document().clone();
    relocated.assets.instances[1].node = 2;
    assert_eq!(
        relocated.assets.instances[1].source_node_index,
        candidate.document().assets.instances[1].source_node_index,
        "only the node placing the prop may differ"
    );
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate {
                document: relocated
            },
            &plan
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "instance_node_mismatch"
        }
    );
}

#[test]
fn a_rest_bind_candidate_that_changes_an_unaffected_world_rest_is_refused() {
    let (doc, prop) = compensated_document_with_unskinned_prop();
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    assert!(!plan.affected_nodes().contains(&prop));

    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    prove_scale(&doc, &candidate, &plan).unwrap();

    // Exercise all four columns of the derived world affine. Comparing
    // only the origin would miss the rotation and scale mutations; a
    // translation-only residual is therefore not this obligation. The
    // one-ulp translation mutation is far inside the v3 scalar band and
    // pins why an unchanged composition has no nonzero tolerance.
    for mutate in [
        |rest: &mut Transform| rest.translation.x = 500.0,
        |rest: &mut Transform| {
            rest.translation.x = f32::from_bits(rest.translation.x.to_bits() + 1)
        },
        |rest: &mut Transform| rest.rotation = Quat::from_rotation_z(0.5),
        |rest: &mut Transform| rest.scale.y = 2.0,
    ] as [fn(&mut Transform); 4]
    {
        let mut changed = candidate.document().clone();
        mutate(&mut changed.skeleton.bones[prop].rest);
        assert_eq!(
            prove_scale(&doc, &ScaleCandidate { document: changed }, &plan).unwrap_err(),
            ScaleError::CandidateStructureMismatch {
                reason: "unaffected_world_rest_mismatch"
            }
        );
    }
}

#[test]
fn a_coherently_reparented_candidate_is_a_topology_mismatch() {
    let (doc, prop) = compensated_document_with_unskinned_prop();
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut reparented = candidate.document().clone();

    let new_parent_source = reparented
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.bone == Some(2))
        .map(|node| node.source_node_index)
        .unwrap();
    reparented.skeleton.bones[prop].parent = Some(2);
    reparented
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.bone == Some(prop))
        .unwrap()
        .parent_source_node_index = Some(new_parent_source);

    // The candidate remains internally coherent, so #309's per-document
    // chain validation accepts it. It is the exact source/candidate
    // topology comparison that must refuse the operation rewrite.
    validate_scale_input(&reparented).unwrap();
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate {
                document: reparented
            },
            &plan
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "skeleton_topology_mismatch"
        }
    );
}

#[test]
fn topology_remains_exact_when_whole_document_affects_every_bone() {
    let (mut doc, prop) = compensated_document_with_unskinned_prop();
    doc.assets.source_skeleton.coverage = SourceSkeletonCoverage::Unavailable;
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert_eq!(plan.affected_nodes(), &[0, 1, 2, 3]);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    let mut changed = candidate.document().clone();
    // Both sides declare their raw rows non-authoritative, so this
    // refusal can only come from normalized `Bone::parent` parity.
    changed.skeleton.bones[prop].parent = Some(2);
    validate_scale_input(&changed).unwrap();

    assert_eq!(
        prove_scale(&doc, &ScaleCandidate { document: changed }, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "skeleton_topology_mismatch"
        }
    );
}

#[test]
fn source_projection_identity_cannot_be_downgraded_by_the_candidate() {
    let (doc, _) = compensated_document_with_unskinned_prop();
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut changed = candidate.document().clone();
    changed.assets.source_skeleton.coverage = SourceSkeletonCoverage::Unavailable;
    // Coverage unavailable deliberately makes the candidate's projection
    // non-authoritative to its own validation. It must not make that
    // identity disappear from the source/candidate comparison.
    validate_scale_input(&changed).unwrap();
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate { document: changed }, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "skeleton_topology_mismatch"
        }
    );
}

#[test]
fn complete_projection_row_identity_is_compared_independently_of_bone_parents() {
    let (doc, _) = compensated_document_with_unskinned_prop();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut changed = candidate.into_document();

    // Add a valid unnormalized raw node. It changes no normalized parent,
    // so only the Complete projection-map comparison can detect that the
    // candidate rewrote source identity outside either operation's set.
    changed.assets.source_skeleton.nodes.push(SourceNodeAsset {
        source_node_index: 100,
        name: None,
        parent_source_node_index: None,
        scene_root_indices: vec![0],
        local_rest: SourceNodeLocalRest::Trs {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
        bone: None,
    });
    validate_scale_input(&changed).unwrap();
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate { document: changed }, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "skeleton_topology_mismatch"
        }
    );
}

#[test]
fn complete_projection_raw_parents_are_compared_independently_of_bone_parents() {
    let (mut doc, _) = compensated_document_with_unskinned_prop();
    for source_node_index in [100, 101] {
        doc.assets.source_skeleton.nodes.push(SourceNodeAsset {
            source_node_index,
            name: None,
            parent_source_node_index: None,
            scene_root_indices: vec![0],
            local_rest: SourceNodeLocalRest::Trs {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            bone: None,
        });
    }
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut changed = candidate.into_document();

    // Both rows remain unprojected, so normalized topology is identical.
    // Only the authoritative raw parent tuple changes.
    changed
        .assets
        .source_skeleton
        .nodes
        .iter_mut()
        .find(|node| node.source_node_index == 101)
        .unwrap()
        .parent_source_node_index = Some(100);
    validate_scale_input(&changed).unwrap();
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate { document: changed }, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "skeleton_topology_mismatch"
        }
    );
}

#[test]
fn complete_projection_bone_identity_is_compared_between_same_parent_siblings() {
    let doc = rig_document(
        &[
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::ZERO),
            rig(Some(0), 2, Vec3::ZERO),
        ],
        &[1],
        0,
        Mat4::IDENTITY,
    );
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut changed = candidate.into_document();

    // Swapping projected identities between siblings preserves both
    // normalized parent values and raw parent values. Only the raw-node
    // to BoneId tuple comparison can distinguish it.
    for node in &mut changed.assets.source_skeleton.nodes {
        node.bone = match node.source_node_index {
            1 => Some(2),
            2 => Some(1),
            _ => node.bone,
        };
    }
    validate_scale_input(&changed).unwrap();
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate { document: changed }, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "skeleton_topology_mismatch"
        }
    );
}

#[test]
fn complete_projection_rows_are_keyed_by_source_identity_not_array_order() {
    let (doc, _) = compensated_document_with_unskinned_prop();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut reordered = candidate.into_document();
    reordered.assets.source_skeleton.nodes.reverse();

    validate_scale_input(&reordered).unwrap();
    prove_scale(
        &doc,
        &ScaleCandidate {
            document: reordered,
        },
        &plan,
    )
    .unwrap();
}

#[test]
fn unavailable_projection_rows_are_not_candidate_identity_evidence() {
    let (mut doc, _) = compensated_document_with_unskinned_prop();
    doc.assets.source_skeleton.coverage = SourceSkeletonCoverage::Unavailable;
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();

    // Under Unavailable coverage the rows are explicitly not a claim
    // about source-node identity. A synthesizing producer may therefore
    // omit stale rows without changing the normalized skeleton topology
    // or any whole-document semantic result.
    let mut without_rows = candidate.into_document();
    without_rows.assets.source_skeleton.nodes.clear();
    prove_scale(
        &doc,
        &ScaleCandidate {
            document: without_rows,
        },
        &plan,
    )
    .unwrap();
}

#[test]
fn unavailable_source_coverage_cannot_be_upgraded_by_the_candidate() {
    let (mut doc, _) = compensated_document_with_unskinned_prop();
    doc.assets.source_skeleton.coverage = SourceSkeletonCoverage::Unavailable;
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let mut upgraded = candidate.into_document();
    upgraded.assets.source_skeleton.coverage = SourceSkeletonCoverage::Complete;

    // The retained rows happen to be coherent, so the candidate is valid
    // in isolation; source/candidate coverage identity must refuse the
    // unilateral upgrade.
    validate_scale_input(&upgraded).unwrap();
    assert_eq!(
        prove_scale(&doc, &ScaleCandidate { document: upgraded }, &plan).unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "skeleton_topology_mismatch"
        }
    );
}

#[test]
fn a_mesh_instance_node_outside_the_skeleton_is_refused_on_either_document() {
    // `node` is resolved downstream without a second bounds test, so an
    // index past the end of the skeleton has to be refused here or it
    // reaches the writer. Doctored on the *second* of two instances, so
    // the check is proved to walk the whole list and to report the slot
    // that actually carries the bad index rather than a fixed 0.
    let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    let twin = MeshInstance {
        source_node_index: 0,
        node: 0,
        ..doc.assets.instances[0].clone()
    };
    doc.assets.instances.push(twin);
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let expected = ScaleError::InvalidDocumentShape(DocumentShapeError::MeshInstanceShape {
        instance_index: 1,
        violation: MeshInstanceShapeViolation::NodeIndexOutOfRange,
    });

    let mut broken_source = doc.clone();
    broken_source.assets.instances[1].node = doc.skeleton.bones.len();
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &broken_source,
            capability: &capability,
        })
        .unwrap_err(),
        expected
    );
    assert_eq!(
        prove_scale(&broken_source, &candidate, &plan).unwrap_err(),
        expected
    );

    let mut broken_candidate = candidate.document().clone();
    broken_candidate.assets.instances[1].node = doc.skeleton.bones.len();
    assert_eq!(
        prove_scale(
            &doc,
            &ScaleCandidate {
                document: broken_candidate
            },
            &plan
        )
        .unwrap_err(),
        expected
    );
}

// --- Per-residual comparison counts (issue #319) ----------------------

/// Every residual's comparison count, named, in the order [`ScaleProof`]
/// declares them.
///
/// Named rather than a bare `[usize; 12]` so a failing vector says which
/// residual moved, and taken as a whole rather than one field per
/// assertion so a document's coverage is stated as one hand-derived fact
/// — including the zeros, which are the half a per-field assertion tends
/// to leave out.
fn comparison_counts(proof: &ScaleProof) -> [(&'static str, usize); 12] {
    [
        ("rest_translation", proof.rest_translation.comparisons()),
        ("rest_rotation", proof.rest_rotation.comparisons()),
        ("unit_scale", proof.unit_scale.comparisons()),
        (
            "transform_only_affine",
            proof.transform_only_affine.comparisons(),
        ),
        ("track_value", proof.track_value.comparisons()),
        ("mesh_position", proof.mesh_position.comparisons()),
        ("key_translation", proof.key_translation.comparisons()),
        ("cubic_interior", proof.cubic_interior.comparisons()),
        ("trajectory", proof.trajectory.comparisons()),
        ("skin_matrix", proof.skin_matrix.comparisons()),
        ("bounds", proof.bounds.comparisons()),
        (
            "unaffected_inverse_bind",
            proof.unaffected_inverse_bind.comparisons(),
        ),
    ]
}

/// The one binary32 rounding a `0.01` factor costs at a coordinate of
/// `1.0`, which is the largest source magnitude in the fixtures below
/// whose product with `0.01` is not exactly representable:
///
/// ```text
/// f32(0.01) = 0.00999999977648258209228515625
/// f64(0.01) = 0.010000000000000000208166817117216851...
/// difference  2.2351741811588166e-10
/// ```
///
/// Every builder in this module narrows to `f32` exactly once per
/// element, and every proof-side expectation is formed in `f64` from the
/// unrounded source, so this is the residual a *correct* candidate
/// carries — not a defect, and not something a rest/bind plan can
/// produce, whose expectation for these domains is `before` exactly.
const NARROWING_AT_ONE: f64 = 2.2351741811588166e-10;

/// `payload_document` plus an affected **cubic** translation track, so
/// one whole-document plan declares all five sampled obligations at once
/// and every domain a whole-document conversion rewrites is measured.
///
/// The track is deliberately constant with zero tangents: at `u = 0.5`
/// the glTF Hermite basis is `h00 = h01 = 0.5` and `h10 = h11 = 0`, so
/// the interior sample is `0.5 * p + 0.5 * p = p` **exactly** on both
/// sides, in `f32`, and the interior-time residual is the same single
/// narrowing as the key-time one rather than an accumulation of spline
/// arithmetic this test would have to re-derive.
fn sampled_payload_document() -> Document {
    let mut doc = payload_document();
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Translation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![
            Vec3::ZERO,               // in-tangent @0
            Vec3::new(0.0, 1.0, 0.0), // value @0
            Vec3::ZERO,               // out-tangent @0
            Vec3::ZERO,               // in-tangent @1
            Vec3::new(0.0, 1.0, 0.0), // value @1
            Vec3::ZERO,               // out-tangent @1
        ]),
    });
    doc
}

#[test]
fn a_whole_document_proof_counts_and_measures_every_comparison_it_makes() {
    let doc = sampled_payload_document();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();

    // Two bones, three mesh vertices, one skin slot; a rotation track and
    // a cubic translation track sharing key times `{0, 1}`, whose single
    // cubic segment contributes the interior time `0.5`.
    assert_eq!(proof.sample_time_count, 3);
    assert_eq!(
        comparison_counts(&proof),
        [
            // One per affected node, and a whole-document closure is
            // every bone.
            ("rest_translation", 2),
            ("rest_rotation", 2),
            // A whole-document plan declares neither the rest/bind
            // postcondition nor a transform-only attachment, so both
            // report the `0.0` that only a count distinguishes from a
            // measured one.
            ("unit_scale", 0),
            ("transform_only_affine", 0),
            // Two rotation values, plus the cubic track's six elements
            // (two values and four tangents).
            ("track_value", 8),
            ("mesh_position", 3),
            // One affected translation track, read at each of the two
            // key times and at the one interior time.
            ("key_translation", 2),
            ("cubic_interior", 1),
            // Both affected nodes, posed at each of the three sample
            // times.
            ("trajectory", 6),
            // One skin slot, at rest and at each of the three sample
            // times. The rest pose is what makes this four rather than
            // three.
            ("skin_matrix", 4),
            // Six per pose: three axes of each of the two extreme
            // corners.
            ("bounds", 24),
            // The document's only skinned instance is bound to bone 1,
            // which a whole-document closure contains.
            ("unaffected_inverse_bind", 0),
        ]
    );

    // Hand-computed residuals, from the fixture's own coordinates. Each
    // is the narrowing above at the magnitude that comparison reads.
    //
    // Bone 1 sits at `(0, 1, 0)`; the candidate holds `(0, f32(0.01), 0)`
    // against an `f64` expectation of `(0, 0.01, 0)`, and a vector with
    // one non-zero component has that component's magnitude for a length.
    assert_eq!(proof.rest_translation.max(), NARROWING_AT_ONE);
    // The same node, posed through the clip: its own rotation track does
    // not move its translation column, and the translation track samples
    // to the same `(0, 1, 0)` at every one of the three times.
    assert_eq!(proof.trajectory.max(), NARROWING_AT_ONE);
    // The track's own elements: the four tangents are `0` (exact under
    // any factor) and both values are `(0, 1, 0)`.
    assert_eq!(proof.track_value.max(), NARROWING_AT_ONE);
    assert_eq!(proof.key_translation.max(), NARROWING_AT_ONE);
    assert_eq!(proof.cubic_interior.max(), NARROWING_AT_ONE);
    // `mesh_position` is a per-vertex L2 norm, and this rig's extreme
    // vertices are `(1, 1, 1)` and `(-1, -1, -1)`: three components each
    // carrying the same narrowing, so `sqrt(3 * e^2)`.
    assert_eq!(proof.mesh_position.max(), 3.871435245533232e-10);
    // Bounds are per axis, and the skinned maximum is at `y = 2`: the
    // vertex `(1, 1, 1)` skinned through `W = translate(0, 1, 0)`. Both
    // `0.02` and `2 * f32(0.01)` are exact doublings of their `y = 1`
    // counterparts, so the residual there is exactly twice the
    // narrowing — which is also what makes this a maximum over the six
    // axis comparisons rather than the only non-zero one.
    assert_eq!(proof.bounds.max(), 2.0 * NARROWING_AT_ONE);
    // A *measured* zero, and the reason the counts exist. The skin
    // equation's expectation is `scale_translation_only(W * B, f32(q))`
    // and the candidate composes `W' * B'` out of the same two `f32`
    // products, so the two sides agree bit for bit — an exact zero that
    // `skin_matrix.comparisons() = 4` distinguishes from the four residuals
    // above it that nothing walked.
    assert_eq!(proof.skin_matrix.max(), 0.0);
    // Likewise exact, and structurally so: neither builder writes
    // `Bone::rest.rotation`, and `quat_equality_residual` of a
    // bit-identical pair is `0.0` by construction. No correct candidate
    // can make this one non-zero; `a_genuinely_rewritten_rest_rotation
    // _still_fails_proof` covers the incorrect ones.
    assert_eq!(proof.rest_rotation.max(), 0.0);
}

#[test]
fn an_unanimated_skinned_document_still_compares_its_skin_at_rest() {
    // The permanent pin for what #317 was closed over: `SkinAndBounds`
    // arms a **rest-pose** walk as well as the sampled
    // loop, so a skinned document with no clips at all still compares its
    // skin matrices and its bounds. Gating those two obligations on the
    // presence of sample times — which that issue proposed — would leave
    // this document's only check unperformed while still publishing a
    // `0.0` for it.
    let doc = compensated_document();
    assert!(doc.clips.is_empty());
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    assert_eq!(plan.transform_only_attachments(), &[2]);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();

    assert_eq!(proof.sample_time_count, 0);
    assert!(!proof.track_value.evaluated());
    assert!(proof.mesh_position.evaluated());
    assert_eq!(
        comparison_counts(&proof),
        [
            ("rest_translation", 3),
            ("rest_rotation", 3),
            // Rest/bind declares the postcondition, over the same three
            // nodes.
            ("unit_scale", 3),
            // One probe point through node 2's expected world affine.
            ("transform_only_affine", 1),
            // No clips, so nothing to compare element-wise and no sample
            // time to pose either skeleton at.
            ("track_value", 0),
            ("mesh_position", 1),
            ("key_translation", 0),
            ("cubic_interior", 0),
            ("trajectory", 0),
            // The whole point: one skin slot and one bounding box,
            // compared at rest, with no sample time in the document.
            ("skin_matrix", 1),
            ("bounds", 6),
            // The document's only skin is inside the closure.
            ("unaffected_inverse_bind", 0),
        ]
    );
}

#[test]
fn a_rest_bind_proof_counts_the_comparisons_its_own_obligations_walk() {
    let doc = multi_joint_document();
    let capability = complete_capability();
    let plan = multi_joint_plan(&doc, &capability);
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    // Both non-root nodes are skin joints, so the closure holds no
    // transform-only attachment and that obligation is not declared.
    assert!(plan.transform_only_attachments().is_empty());
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();

    // Key times `{0, 1}` shared by both tracks, plus the cubic track's
    // one interior time `0.5`.
    assert_eq!(proof.sample_time_count, 3);
    assert_eq!(
        comparison_counts(&proof),
        [
            ("rest_translation", 3),
            ("rest_rotation", 3),
            ("unit_scale", 3),
            ("transform_only_affine", 0),
            // Two linear translation values plus six cubic elements.
            ("track_value", 8),
            ("mesh_position", 4),
            // Two affected translation tracks, at each of the two key
            // times and at the one interior time.
            ("key_translation", 4),
            ("cubic_interior", 2),
            // Three affected nodes at each of the three sample times.
            ("trajectory", 9),
            // Two skin slots, at rest and at each of the three sample
            // times.
            ("skin_matrix", 8),
            ("bounds", 24),
            ("unaffected_inverse_bind", 0),
        ]
    );
}

#[test]
fn a_clipless_plan_compares_no_track_value_while_still_comparing_its_mesh() {
    // This document carries a mesh and a skin but no clip, so it declares
    // no `TrackValues` claim and the row-driven comparison has nothing to
    // walk while the mesh claim still does — which is what separates
    // `track_value`'s count from `mesh_position`'s rather than leaving
    // both to move together.
    let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
    assert!(doc.clips.is_empty());
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();

    assert_eq!(proof.sample_time_count, 0);
    assert_eq!(
        comparison_counts(&proof),
        [
            ("rest_translation", 2),
            ("rest_rotation", 2),
            ("unit_scale", 0),
            ("transform_only_affine", 0),
            ("track_value", 0),
            ("mesh_position", 1),
            ("key_translation", 0),
            ("cubic_interior", 0),
            ("trajectory", 0),
            ("skin_matrix", 1),
            ("bounds", 6),
            ("unaffected_inverse_bind", 0),
        ]
    );
}

#[test]
fn a_meshless_plan_compares_no_mesh_position_while_still_comparing_its_tracks() {
    // The other half: clips but no mesh. The two documents together are
    // what make `track_value` and `mesh_position` independently
    // observable — on every other fixture in this module they are either
    // both walked or both empty.
    let mut doc = payload_document();
    doc.assets.meshes.clear();
    doc.assets.instances.clear();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();

    assert_eq!(proof.sample_time_count, 2);
    assert_eq!(
        comparison_counts(&proof),
        [
            ("rest_translation", 2),
            ("rest_rotation", 2),
            ("unit_scale", 0),
            ("transform_only_affine", 0),
            // The rotation track's two values.
            ("track_value", 2),
            ("mesh_position", 0),
            // A rotation track evidences trajectories but neither key
            // translations nor cubic interiors.
            ("key_translation", 0),
            ("cubic_interior", 0),
            ("trajectory", 4),
            // With the instance gone there is no skinned evidence, so the
            // combined skin/bounds obligation is not declared.
            ("skin_matrix", 0),
            ("bounds", 0),
            ("unaffected_inverse_bind", 0),
        ]
    );
}

#[test]
fn a_skin_outside_the_closure_is_the_only_source_of_an_unaffected_bind_comparison() {
    // The third row-driven claim, and the one whose producer-side
    // predicate used to hand-mirror `stored_instance_bind`'s private
    // resolution order across a crate boundary. Counting at the point of
    // comparison is what removes that mirror: this document's second
    // instance is bound to bone 3, outside the closure, and stores a bind
    // on both sides.
    let doc = compensated_document_with_unrelated_skin(Some(Mat4::from_scale(Vec3::splat(2.0))));
    let capability = complete_capability();
    let plan = compensated_rest_bind_plan(&doc, &capability);
    assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();

    // One slot of one instance outside the closure.
    assert_eq!(proof.unaffected_inverse_bind.comparisons(), 1);
    // A measured zero: rest/bind must leave an unrelated skin's stored
    // bind exactly as it found it, so the correct candidate's residual
    // here is `0.0` — indistinguishable, without the count, from the `0.0`
    // `compensated_document` reports for having no such skin at all.
    assert_eq!(proof.unaffected_inverse_bind.max(), 0.0);
    // The extra bone is outside the closure, so the rest walk is
    // unchanged, and the extra instance is not the skin obligation's.
    assert_eq!(proof.rest_translation.comparisons(), 3);
    assert_eq!(proof.skin_matrix.comparisons(), 1);
}

#[test]
fn an_unskinned_document_compares_neither_skin_matrices_nor_bounds() {
    let doc = unskinned_document();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    assert!(
        !plan
            .obligations()
            .contains(&ScaleProofObligation::SkinAndBounds)
    );
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();

    assert_eq!(
        comparison_counts(&proof),
        [
            ("rest_translation", 1),
            ("rest_rotation", 1),
            ("unit_scale", 0),
            ("transform_only_affine", 0),
            ("track_value", 0),
            // The unskinned instance's base positions are still proved
            // directly; the shared skin/bounds walk cannot make this claim.
            ("mesh_position", 1),
            ("key_translation", 0),
            ("cubic_interior", 0),
            ("trajectory", 0),
            ("skin_matrix", 0),
            ("bounds", 0),
            ("unaffected_inverse_bind", 0),
        ]
    );
}

#[test]
fn a_boneless_whole_document_plan_compares_nothing_at_all() {
    // The zero row of the matrix, all twelve at once: an empty document
    // plans an empty closure and carries no row data or sampled evidence
    // for any numeric claim to walk. Every residual it publishes is `0.0`,
    // and every count says why.
    let doc = Document::default();
    let capability = complete_capability();
    let plan = whole_document_plan(&doc, &capability);
    let obligations = plan.obligations().to_vec();
    assert!(plan.rest_obligation().is_none());
    assert!(!obligations.contains(&ScaleProofObligation::KeyTranslations));
    let candidate = build_scale_candidate(&doc, &plan).unwrap();
    let proof = prove_scale(&doc, &candidate, &plan).unwrap();

    assert_eq!(proof.sample_time_count, 0);
    assert_eq!(
        comparison_counts(&proof).map(|(_, count)| count),
        [0usize; 12]
    );
}

// ----------------------------------------------------------------------
// The calibration sweep behind `ScaleTolerancePolicy::f32_rounding_ulps`.
// ----------------------------------------------------------------------

/// A deterministic 64-bit generator for [`calibrate_f32_rounding_ulps`].
///
/// SplitMix64, written out rather than depended on: one `u64` of state, the
/// same stream on every platform and every run, and no dependency added to
/// a crate whose whole dependency set is `glam + serde + sha2 + thiserror`.
/// Determinism is the point — a calibration whose population changes per
/// run is a number no reader can re-derive.
///
/// The *bit stream* is platform-independent; the rigs drawn from it are
/// not quite. [`Self::decades`] runs `f64::powf`, [`Self::direction`] runs
/// `f64::sin`/`cos`, and [`Self::rotation`] runs glam's `f32` sine and
/// cosine, none of which is required to be correctly rounded. Two machines
/// can therefore differ in the last ulp of a joint local or a rotation, and
/// the worst demand each reports can differ in its last printed digit. That
/// is the resolution at which these figures are re-derivable — at `2.8`
/// against a count of `4` it cannot move a verdict, and the assertions
/// below are thresholds rather than equalities for that reason.
struct SweepRng(u64);

impl SweepRng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Log-uniform in `[10^lo, 10^hi]`, which is how a magnitude sweep
    /// covers decades rather than covering the top decade twelve times.
    fn decades(&mut self, lo: f64, hi: f64) -> f32 {
        10f64.powf(lo + self.unit() * (hi - lo)) as f32
    }

    /// Uniform on the unit sphere.
    fn direction(&mut self) -> Vec3 {
        let z = 2.0 * self.unit() - 1.0;
        let angle = core::f64::consts::TAU * self.unit();
        let radius = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(
            (radius * angle.cos()) as f32,
            (radius * angle.sin()) as f32,
            z as f32,
        )
        .normalize()
    }

    /// Uniform on the rotation group.
    fn rotation(&mut self) -> Quat {
        Quat::from_axis_angle(
            self.direction(),
            (core::f64::consts::TAU * self.unit()) as f32,
        )
    }
}

/// What a sweep cell composes each skin slot to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SweepComposition {
    /// Each bind is the analytic inverse of its own rest world, so every
    /// `W * B` is the identity. This is what every fixture built before
    /// this sweep existed, and it is the shape that cannot cancel.
    Analytic,
    /// Each slot composes to `10^exponent` times a random rotation, so
    /// `abs(W * B)` is `10^exponent` and not `1`. Both signs of the
    /// exponent are swept, because a composed slot that *shrinks* the
    /// geometry it carries is as reachable as one that grows it.
    Scaled(i32),
}

/// Whether a cell's two slots oppose on the swept vertex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SweepBlend {
    /// The second slot is the first composed with [`HALF_TURN_Z`] and the
    /// vertex lies along `x`, so the two terms of the weighted sum are
    /// exact negations and the blend cancels to the origin. The result and
    /// earlier slot stages then contain no record of the transform's own
    /// operands.
    Cancelling,
    /// The two slots are independent, so the blend keeps the magnitude it
    /// was built from.
    Independent,
}

/// The skin-weight/provenance relationship a cell exercises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SweepWeights {
    /// Both slots contribute equally.
    Balanced,
    /// For each vertex whose two influence bases differ, the larger base
    /// receives a log-uniform weight in `[1e-20, 1e-2]` and the smaller
    /// receives `1`.
    Mismatched,
}

/// One sweep cell: an operation, a slot composition, and a blend.
#[derive(Debug, Clone, Copy)]
struct SweepCell {
    /// `None` for rest/bind at root scale `3190`; `Some(q)` for a
    /// whole-document conversion at `q` from a root scale of `1`.
    conversion: Option<f64>,
    composition: SweepComposition,
    blend: SweepBlend,
    weights: SweepWeights,
}

impl SweepCell {
    /// A seed derived from every coordinate of the cell, so a cell's
    /// population is a function of what the cell *is* and does not move
    /// when a neighbouring cell is added, removed, or reordered.
    ///
    /// An earlier revision mixed the running loop ordinal and the
    /// conversion factor only, and claimed this property while not having
    /// it: swapping the two blends, or dropping a composition, moved the
    /// population of every cell after the change and so silently
    /// re-measured the whole sweep.
    fn seed(self) -> u64 {
        // Each coordinate is folded through the generator's own mixing
        // step rather than xored in, so no two cells can collide on a
        // seed by an accident of how the coordinates are encoded.
        let mut state = 0x5EED_0000_0000_0000u64;
        for word in [
            self.conversion.unwrap_or(0.0).to_bits(),
            match self.composition {
                SweepComposition::Analytic => 0,
                // Offset past the `Analytic` marker, and biased so a
                // negative exponent stays inside the `u64`.
                SweepComposition::Scaled(exponent) => 1 + (i64::from(exponent) + 1024) as u64,
            },
            match self.blend {
                SweepBlend::Cancelling => 0,
                SweepBlend::Independent => 1,
            },
            match self.weights {
                SweepWeights::Balanced => 0,
                SweepWeights::Mismatched => 1,
            },
        ] {
            state = SweepRng(state ^ word).next_u64();
        }
        state
    }
}

/// The worst ulp count a cell asked of the rounding term, per obligation.
#[derive(Debug, Clone, Copy, Default)]
struct SweepWorst {
    bounds: f64,
    skin_matrix: f64,
    rest_translation: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct SweepSample {
    worst: SweepWorst,
    mismatched_vertices: usize,
    larger_slot_zero: usize,
    larger_slot_one: usize,
}

impl SweepWorst {
    fn fold(&mut self, other: Self) {
        self.bounds = self.bounds.max(other.bounds);
        self.skin_matrix = self.skin_matrix.max(other.skin_matrix);
        self.rest_translation = self.rest_translation.max(other.rest_translation);
    }
}

/// Per-comparison deep-chain demand. Unlike [`SweepWorst`], these values
/// never divide an obligation-wide maximum residual by a separately
/// selected maximum base: each ratio is formed at the production comparison
/// site that owns both quantities, and only then folded.
#[derive(Debug, Clone, Copy, Default)]
struct DeepChainWorst {
    rest_translation: f64,
    trajectory: f64,
    skin_matrix: f64,
    bounds: f64,
    rest_comparisons: usize,
    trajectory_comparisons: usize,
    skin_comparisons: usize,
    bounds_comparisons: usize,
}

impl DeepChainWorst {
    fn fold(&mut self, other: Self) {
        self.rest_translation = self.rest_translation.max(other.rest_translation);
        self.trajectory = self.trajectory.max(other.trajectory);
        self.skin_matrix = self.skin_matrix.max(other.skin_matrix);
        self.bounds = self.bounds.max(other.bounds);
        self.rest_comparisons += other.rest_comparisons;
        self.trajectory_comparisons += other.trajectory_comparisons;
        self.skin_comparisons += other.skin_comparisons;
        self.bounds_comparisons += other.bounds_comparisons;
    }

    /// The deep calibration consumes the exact per-comparison f32 demand
    /// maxima and counts the production proof accumulated.
    fn from_proof(proof: ScaleProof) -> Self {
        Self {
            rest_translation: proof.rest_translation_f32_rounding_demand,
            trajectory: proof.trajectory_f32_rounding_demand,
            skin_matrix: proof.skin_matrix_f32_rounding_demand,
            bounds: proof.bounds_f32_rounding_demand,
            rest_comparisons: proof.rest_translation.comparisons(),
            trajectory_comparisons: proof.trajectory.comparisons(),
            skin_comparisons: proof.skin_matrix.comparisons(),
            bounds_comparisons: proof.bounds.comparisons(),
        }
    }
}

#[test]
fn calibration_demand_ownership_is_not_swappable() {
    let (doc, plan, candidate) = animated_deep_chain_conversion(8);
    let mut proof = prove_scale(&doc, &candidate, &plan).expect("the calibration rig proves");
    proof.rest_translation_f32_rounding_demand = 0.0;
    proof.trajectory_f32_rounding_demand = 0.0;
    proof.skin_matrix_f32_rounding_demand = 0.0;
    proof.bounds_f32_rounding_demand = 0.0;
    proof.unaffected_inverse_bind_f32_rounding_demand = 0.0;

    let epsilon = f64::from(f32::EPSILON);
    for (kind, demand) in [
        (ProofResidualKind::RestTranslation, 1.0),
        (ProofResidualKind::Trajectory, 2.0),
        (ProofResidualKind::SkinMatrix, 3.0),
        (ProofResidualKind::Bounds, 4.0),
        (ProofResidualKind::UnaffectedInverseBind, 5.0),
    ] {
        proof.record_f32_rounding_demand(kind, demand * epsilon, 1.0);
    }
    assert_eq!(
        (
            proof.rest_translation_f32_rounding_demand,
            proof.trajectory_f32_rounding_demand,
            proof.skin_matrix_f32_rounding_demand,
            proof.bounds_f32_rounding_demand,
            proof.unaffected_inverse_bind_f32_rounding_demand,
        ),
        (1.0, 2.0, 3.0, 4.0, 5.0),
        "the central production diagnostic must keep every residual kind in its own field",
    );
    proof.record_f32_rounding_demand(ProofResidualKind::UnaffectedInverseBind, 1.0, 0.0);
    assert!(
        proof
            .unaffected_inverse_bind_f32_rounding_demand
            .is_infinite(),
        "a nonzero residual with no rounding provenance must fail calibration closed",
    );

    let expected_counts = (
        proof.rest_translation.comparisons(),
        proof.trajectory.comparisons(),
        proof.skin_matrix.comparisons(),
        proof.bounds.comparisons(),
    );
    let measured = DeepChainWorst::from_proof(proof);
    assert_eq!(
        (
            measured.rest_translation,
            measured.trajectory,
            measured.skin_matrix,
            measured.bounds,
        ),
        (1.0, 2.0, 3.0, 4.0),
        "the deep calibration adapter must not swap or duplicate obligation demands",
    );
    assert_eq!(
        (
            measured.rest_comparisons,
            measured.trajectory_comparisons,
            measured.skin_comparisons,
            measured.bounds_comparisons,
        ),
        expected_counts,
        "the deep calibration adapter must retain each obligation's production count",
    );
}

fn deep_chain_case(
    depth: usize,
    rotation: Quat,
    conversion: Option<f64>,
) -> Result<DeepChainWorst, ScaleError> {
    let root_scale = if conversion.is_some() { 1.0 } else { 3190.0 };
    let doc = chain_document(depth, rotation, root_scale, true);
    let plan = match conversion {
        Some(factor) => plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor },
            document: &doc,
            capability: &complete_capability(),
        })?,
        None => rest_bind_plan(&doc, f64::from(root_scale)),
    };
    let candidate = build_scale_candidate(&doc, &plan)?;
    let proof = prove_scale(&doc, &candidate, &plan)?;
    assert_eq!(
        proof.sample_time_count, 2,
        "every animated deep calibration case must retain two production sample times",
    );
    Ok(DeepChainWorst::from_proof(proof))
}

/// Build one sweep candidate, prove it, and return the ulps each residual
/// asked of its own comparison base.
///
/// The primary shipped-base maxima come from the production proof's
/// exact f32-rounded comparisons, so calibration cannot quote a base that
/// proof stopped using.
///
/// The quantity is `residual / (base * 2^-23)`: the raw ulp count, *not*
/// net of the scalar band that is paid first. It therefore overstates what
/// the count is actually asked for, by the whole scalar band, and a worst
/// case under `4` measured this way is a worst case under `4` however the
/// two terms are split.
fn sweep_one(rng: &mut SweepRng, cell: SweepCell) -> Result<SweepSample, ScaleError> {
    let root = if cell.conversion.is_some() {
        1.0
    } else {
        3190.0
    };
    let rotations = [rng.rotation(), rng.rotation()];
    let mut locals = [
        rng.direction() * rng.decades(-3.0, 3.0),
        rng.direction() * rng.decades(-3.0, 3.0),
    ];
    // Half of every cell's trials cancel the parent chain: the second
    // joint's local offset points back along its parent's world
    // translation, so its world translation is the difference of two terms
    // of the first local's magnitude and the surviving translation is a
    // rounding artefact of it. That is the shape `RestTranslation` and the
    // chain half of `SkinSlot::rounding_magnitude` exist for, and swept
    // inside every cell rather than as a cell of its own so it crosses
    // every operation and every composition.
    if rng.unit() < 0.5 {
        locals[1] = -(rotations[0].inverse() * locals[0]);
    }
    // One vertex along `x` — the axis `HALF_TURN_Z` negates, so a
    // cancelling cell cancels exactly — and one in a random direction, so
    // no cell is a pure `x`-axis population.
    let reach = rng.decades(-3.0, 5.0);
    let points = [
        Vec3::new(reach, 0.0, 0.0),
        rng.direction() * rng.decades(-3.0, 5.0),
    ];
    let weights = [[0.5, 0.5, 0.0, 0.0]; 2];

    let mut doc = match cell.composition {
        SweepComposition::Analytic => {
            rotating_rig_document(rotations, root, locals, &points, &weights)
        }
        SweepComposition::Scaled(exponent) => {
            let first = Mat4::from_scale(Vec3::splat(10f32.powi(exponent)))
                * Mat4::from_quat(rng.rotation());
            let second = match cell.blend {
                SweepBlend::Cancelling => first * HALF_TURN_Z,
                SweepBlend::Independent => {
                    Mat4::from_scale(Vec3::splat(10f32.powi(exponent)))
                        * Mat4::from_quat(rng.rotation())
                }
            };
            composed_slot_document(rotations, root, locals, [first, second], &points, &weights)
        }
    };
    let mut sample = SweepSample::default();
    if cell.weights == SweepWeights::Mismatched {
        let slots = rig_skin_slots(&doc);
        for (vertex, &point) in points.iter().enumerate() {
            let bases = [
                skin_influence_magnitude(&slots[0], point),
                skin_influence_magnitude(&slots[1], point),
            ];
            let Some((larger, smaller)) = (bases[0] > bases[1])
                .then_some((0, 1))
                .or_else(|| (bases[1] > bases[0]).then_some((1, 0)))
            else {
                continue;
            };
            let mut mismatched = [0.0; 4];
            mismatched[larger] = rng.decades(-20.0, -2.0);
            mismatched[smaller] = 1.0;
            doc.assets.meshes[0].primitives[0].weights[vertex] = mismatched;
            let installed = doc.assets.meshes[0].primitives[0].weights[vertex];
            assert!(
                installed[larger] > 0.0
                    && installed[larger] <= 1e-2
                    && installed[larger] < installed[smaller]
                    && installed[smaller] == 1.0,
                "the production document did not retain the requested mismatch: bases \
                     {bases:?}, weights {installed:?}"
            );
            sample.mismatched_vertices += 1;
            sample.larger_slot_zero += usize::from(larger == 0);
            sample.larger_slot_one += usize::from(larger == 1);
        }
    }
    // An analytic cell composes every slot to the identity, so its blend
    // cannot be made to cancel and the two blend values name the same
    // population. Swept anyway rather than special-cased, because "the
    // shape that cannot cancel" is a claim the sweep should be able to
    // contradict if it ever stops being true.
    let plan = match cell.conversion {
        None => rest_bind_plan(&doc, f64::from(root)),
        Some(factor) => plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor },
            document: &doc,
            capability: &complete_capability(),
        })
        .expect("a whole-document conversion plans at any positive factor"),
    };
    let candidate = build_scale_candidate(&doc, &plan).expect("a planned rig builds");
    let proof = prove_scale(&doc, &candidate, &plan)?;

    sample.worst = SweepWorst {
        bounds: proof.bounds_f32_rounding_demand,
        skin_matrix: proof.skin_matrix_f32_rounding_demand,
        rest_translation: proof.rest_translation_f32_rounding_demand,
    };
    Ok(sample)
}

/// The sweep [`ScaleTolerancePolicy::f32_rounding_ulps`] is calibrated
/// from, checked in so the count is re-derivable rather than a constant a
/// reader has to take on trust.
///
/// Run it with
///
/// ```text
/// cargo test -p animsmith-core --release --lib \
///     calibrate_f32_rounding_ulps -- --ignored --nocapture
/// ```
///
/// `#[ignore]`d because it builds and proves 360_000 candidates — seconds
/// in `--release` and minutes in the debug profile `cargo test` uses, where
/// the rest of this module costs milliseconds. It is still *compiled* by
/// every `cargo test`, so it cannot rot away from the code it measures.
///
/// The population is the cross product of
///
/// - nine operations: rest/bind at root scale `3190`, and whole-document
///   conversion at `{1e-4, 0.01, 0.1, 1.5, 7.3, 100, 3190, 1e6}` — both
///   directions of the factor, which is what separates a base read off the
///   candidate from one read off the source;
/// - four slot compositions: analytic binds (`abs(W * B) = 1`, the only
///   shape the pre-sweep fixtures could build) and composed slots at
///   `abs(W * B) = {1e-3, 1, 1e3}`; and
/// - two blends: two slots that oppose on the swept vertex and cancel it
///   to the origin under balanced weights, and two independent slots that
///   do not; and
/// - two explicit weight profiles: balanced `[0.5, 0.5]`, and mismatched,
///   where each vertex's larger production influence base receives a
///   log-uniform weight in `[1e-20, 1e-2]` and the smaller receives `1`.
///
/// with joint locals and vertex positions drawn log-uniformly over six and
/// eight decades in random directions, a random rotation per joint, and
/// half of every cell's trials carrying a parent chain that cancels.
/// Every candidate is *correct* — [`build_scale_candidate`] produced it —
/// so every refusal is a false negative and every ulp count is a demand a
/// correct document made of the count.
///
/// The 144-cell phase is shallow by construction. Its separate deep phase
/// proves 80 animated cases through depth 512, including a literal
/// 192-link ring, and measures RestTranslation, Trajectory, SkinMatrix and
/// Bounds per comparison. See `docs/scale-calibration.md` for the exact
/// population and recorded maxima.
///
/// The assertions are the calibration: no cell may refuse a correct
/// candidate, and no residual may ask more of the production comparison
/// base than [`ScaleTolerancePolicy::f32_rounding_ulps`] allows.
///
/// Five more assertions are the sweep's *floor*, because a sweep that
/// silently measured nothing would otherwise report `0.000` in every
/// column and pass: the cell count and the candidate count are checked
/// against literals rather than against the swept arrays' own lengths, and
/// the mismatch-profile candidate count is fixed, the installed document
/// must actually carry the inverse weight/base relation in both slot
/// orientations within narrow brackets around the recorded population,
/// and each worst demand is bracketed below the shipped count. An earlier
/// revision had none of these and claimed all of them. It compared `cells`
/// against `conversions.len() * compositions.len() * blends.len()`, which
/// holds however far those arrays are cut back — deleting every
/// composition but [`SweepComposition::Analytic`], which is exactly the
/// blindness that hid the transform stage, killed no assertion at all, and
/// neither did `TRIALS = 1`.
///
/// What the floor does *not* buy is the over-acceptance direction. Every
/// base here is a `max` over stages, so loosening one lowers the measured
/// demand toward zero rather than raising it: a base widened inside
/// [`accumulate_skinned_bounds`] is caught by the Bounds non-silence
/// bracket below (it reports `bounds 0.000`), but one widened at the *call
/// site*, where this sweep's own helpers do not read it, moves nothing this
/// test measures. That
/// direction is held by four chain-dominant adjacent-binary32 searches —
/// `the_bounds_v6_floor_is_an_adjacent_f32_transition`,
/// `the_skin_matrix_v6_floor_is_an_adjacent_f32_transition`,
/// `the_rest_translation_v6_floor_is_an_adjacent_f32_transition`, and
/// `the_trajectory_v6_floor_is_an_adjacent_f32_transition` — plus the
/// far-joint and factor-direction brackets. Each names a fixture-local
/// floor, not a universal smallest defect. This sweep is a one-directional
/// instrument and says so.
#[test]
#[ignore = "calibration: 360,000 shallow proofs plus 80 deep cases. See docs/scale-calibration.md."]
fn calibrate_f32_rounding_ulps() {
    const TRIALS: usize = 2_500;
    let conversions = [
        None,
        Some(1e-4),
        Some(0.01),
        Some(0.1),
        Some(1.5),
        Some(7.3),
        Some(100.0),
        Some(3190.0),
        Some(1e6),
    ];
    let compositions = [
        SweepComposition::Analytic,
        SweepComposition::Scaled(-3),
        SweepComposition::Scaled(0),
        SweepComposition::Scaled(3),
    ];
    let blends = [SweepBlend::Cancelling, SweepBlend::Independent];
    let weight_profiles = [SweepWeights::Balanced, SweepWeights::Mismatched];

    let mut overall = SweepWorst::default();
    let mut refusals = Vec::new();
    let mut cells = 0usize;
    let mut mismatched_profile_candidates = 0usize;
    let mut mismatched_vertices = 0usize;
    let mut larger_slot_zero = 0usize;
    let mut larger_slot_one = 0usize;
    println!(
        "{:>8}  {:>10}  {:>12}  {:>10}  {:>8}  {:>8}  {:>8}  {:>8}",
        "conv", "abs(W*B)", "blend", "weights", "refused", "bounds", "skin", "rest"
    );
    for &conversion in &conversions {
        for &composition in &compositions {
            for &blend in &blends {
                for &weights in &weight_profiles {
                    let cell = SweepCell {
                        conversion,
                        composition,
                        blend,
                        weights,
                    };
                    let mut rng = SweepRng(cell.seed());
                    let mut worst = SweepWorst::default();
                    let mut refused = 0usize;
                    for _ in 0..TRIALS {
                        match sweep_one(&mut rng, cell) {
                            Ok(measured) => {
                                worst.fold(measured.worst);
                                mismatched_vertices += measured.mismatched_vertices;
                                larger_slot_zero += measured.larger_slot_zero;
                                larger_slot_one += measured.larger_slot_one;
                            }
                            Err(error) => {
                                refused += 1;
                                if refused == 1 {
                                    refusals.push(format!("{cell:?}: {error:?}"));
                                }
                            }
                        }
                    }
                    println!(
                        "{:>8}  {:>10}  {:>12}  {:>10}  {:>8}  {:>8.3}  {:>8.3}  {:>8.3}",
                        conversion.map_or("rest/bind".into(), |q| format!("{q:e}")),
                        match composition {
                            SweepComposition::Analytic => "1 (exact)".into(),
                            SweepComposition::Scaled(e) => format!("1e{e}"),
                        },
                        format!("{blend:?}"),
                        format!("{weights:?}"),
                        format!("{refused}/{TRIALS}"),
                        worst.bounds,
                        worst.skin_matrix,
                        worst.rest_translation,
                    );
                    overall.fold(worst);
                    cells += 1;
                    mismatched_profile_candidates +=
                        usize::from(weights == SweepWeights::Mismatched) * TRIALS;
                }
            }
        }
    }
    let deep_depths = [8, 16, 32, 64, 128, 192, 256, 512];
    let mut deep_by_depth = [DeepChainWorst::default(); 8];
    let mut deep = DeepChainWorst::default();
    let mut deep_cases = 0usize;
    for (depth_index, &depth) in deep_depths.iter().enumerate() {
        for &conversion in &conversions {
            // Repeated 170-degree quaternion composition accumulates a
            // tiny non-uniform scale by the deepest rest/bind cases, which
            // the affine input boundary correctly refuses. Rest/bind uses
            // an exact half turn instead: still a maximally cancelling
            // chain, but one whose linear part stays exactly supported.
            let rotation = if conversion.is_some() {
                DEEP_CHAIN_ROTATION
            } else {
                Quat::from_xyzw(0.0, 0.0, 1.0, 0.0)
            };
            match deep_chain_case(depth, rotation, conversion) {
                Ok(measured) => {
                    deep_by_depth[depth_index].fold(measured);
                    deep.fold(measured);
                }
                Err(error) => refusals.push(format!(
                    "deep chain depth {depth}, conversion {conversion:?}: {error:?}"
                )),
            }
            deep_cases += 1;
        }
    }
    for &conversion in conversions.iter().flatten() {
        match deep_chain_case(192, RING_CHAIN_ROTATION, Some(conversion)) {
            Ok(measured) => {
                deep_by_depth[5].fold(measured);
                deep.fold(measured);
            }
            Err(error) => refusals.push(format!(
                "ring chain depth 192, conversion {conversion}: {error:?}"
            )),
        }
        deep_cases += 1;
    }
    let total = cells * TRIALS;
    println!(
        "\n{total} correct candidates over {cells} cells; {mismatched_vertices} realized \
             mismatched vertices ({larger_slot_zero} with slot 0 larger, {larger_slot_one} with \
             slot 1 larger); worst ulps of the comparison base: \
             bounds {:.3}, skin matrix {:.3}, rest translation {:.3}; \
             f32_rounding_ulps = {}",
        overall.bounds,
        overall.skin_matrix,
        overall.rest_translation,
        ScaleTolerancePolicy::APPENDIX_D_V6.f32_rounding_ulps,
    );
    println!(
        "deep-chain calibration: {deep_cases} cases through 512 links; worst per-comparison \
             ulps: rest {:.3}, trajectory {:.3}, skin matrix {:.3}, bounds {:.3}; comparisons \
             {}/{}/{}/{}",
        deep.rest_translation,
        deep.trajectory,
        deep.skin_matrix,
        deep.bounds,
        deep.rest_comparisons,
        deep.trajectory_comparisons,
        deep.skin_comparisons,
        deep.bounds_comparisons,
    );
    println!("depth      rest      trajectory      skin      bounds      comparisons r/t/s/b");
    for (depth, measured) in deep_depths.into_iter().zip(deep_by_depth) {
        println!(
            "{depth:>5}  {:>8.3}  {:>14.3}  {:>8.3}  {:>8.3}      {}/{}/{}/{}",
            measured.rest_translation,
            measured.trajectory,
            measured.skin_matrix,
            measured.bounds,
            measured.rest_comparisons,
            measured.trajectory_comparisons,
            measured.skin_comparisons,
            measured.bounds_comparisons,
        );
    }

    assert!(
        refusals.is_empty(),
        "the sweep refused correct candidates, one per cell shown: {refusals:#?}",
    );
    // The population's own floor, as literals. Comparing `cells` against
    // `conversions.len() * compositions.len() * blends.len()` — which is
    // what an earlier revision did — compares the loop counter against the
    // mutated arrays' own lengths and so holds under every shrinkage of
    // them: deleting every composition but `Analytic`, which removes the
    // whole `abs(W * B) != 1` population that the transform-stage defect
    // hid in, left it passing. So did `TRIALS = 1`.
    assert_eq!(
        cells, 144,
        "the sweep no longer runs the 144 cells docs/scale-calibration.md names. If a \
             dimension was deliberately added or removed, this literal and the prose that quotes \
             it move together.",
    );
    assert_eq!(
        cells * TRIALS,
        360_000,
        "the sweep no longer draws the 360_000 candidates docs/scale-calibration.md names, so \
             the figures below are not the ones that note quotes.",
    );
    assert_eq!(
        mismatched_profile_candidates, 180_000,
        "half of the sweep must use the explicit mismatched-weight profile",
    );
    assert_eq!(
        deep_cases, 80,
        "the deep calibration must retain eight declared depths across nine operations plus \
             the eight whole-document conversions of the 192-link ring",
    );
    assert!(
        deep.rest_comparisons > 0
            && deep.trajectory_comparisons > 0
            && deep.skin_comparisons > 0
            && deep.bounds_comparisons > 0,
        "every affected obligation must own measured deep-chain comparisons: {deep:?}",
    );
    assert_eq!(
        (
            deep.rest_comparisons,
            deep.trajectory_comparisons,
            deep.skin_comparisons,
            deep.bounds_comparisons,
        ),
        (12_488, 24_976, 240, 1_440),
        "the deep phase's production comparison counts no longer match \
         docs/scale-calibration.md",
    );
    let deep_counts = deep_by_depth.map(|measured| {
        (
            measured.rest_comparisons,
            measured.trajectory_comparisons,
            measured.skin_comparisons,
            measured.bounds_comparisons,
        )
    });
    assert_eq!(
        deep_counts,
        [
            (81, 162, 27, 162),
            (153, 306, 27, 162),
            (297, 594, 27, 162),
            (585, 1_170, 27, 162),
            (1_161, 2_322, 27, 162),
            (3_281, 6_562, 51, 306),
            (2_313, 4_626, 27, 162),
            (4_617, 9_234, 27, 162),
        ],
        "a declared depth no longer owns the comparison population \
         docs/scale-calibration.md records",
    );
    let deep_demands_milli = deep_by_depth.map(|measured| {
        (
            (measured.rest_translation * 1_000.0).round() as u16,
            (measured.trajectory * 1_000.0).round() as u16,
            (measured.skin_matrix * 1_000.0).round() as u16,
            (measured.bounds * 1_000.0).round() as u16,
        )
    });
    assert_eq!(
        deep_demands_milli,
        [
            (578, 578, 143, 149),
            (578, 578, 67, 73),
            (578, 578, 76, 78),
            (578, 578, 63, 63),
            (578, 578, 37, 39),
            (715, 715, 34, 33),
            (578, 578, 34, 34),
            (578, 578, 19, 19),
        ],
        "the literal deep-chain demand table no longer matches the values recorded to three decimals",
    );
    assert!(
        deep_by_depth.iter().all(|measured| {
            measured.rest_comparisons > 0
                && measured.trajectory_comparisons > 0
                && measured.skin_comparisons > 0
                && measured.bounds_comparisons > 0
        }),
        "every declared depth must record every affected obligation: {deep_by_depth:#?}",
    );
    assert!(
        (273_000..=276_000).contains(&mismatched_vertices)
            && (73_000..=76_000).contains(&larger_slot_zero)
            && (199_000..=202_000).contains(&larger_slot_one),
        "the mismatch profile did not realize both production-base orientations: \
             {mismatched_vertices} vertices total, slot 0 larger {larger_slot_zero}, slot 1 \
             larger {larger_slot_one}; expected the platform-tolerant brackets around the \
             recorded 274670/74085/200585 population",
    );
    // A floor on what the sweep *measured*, not only on what it refused.
    // Every base in this proof grows monotonically with its arithmetic
    // provenance, so loosening any one of them drives the measured demand
    // toward zero rather than toward the count: without this, a mutation
    // that inflates a base
    // reports `0.000` in every column and passes. The over-acceptance
    // direction proper is covered by named fixtures — the bracket tests
    // that pin a fixture-local adjacent transition per obligation — and this
    // is only the floor that stops the sweep from reporting silence as
    // success. The floors sit just below the documented maxima, while the
    // policy count below is their common upper bracket.
    assert!(
        (2.70..2.85).contains(&overall.bounds)
            && (2.10..2.30).contains(&overall.skin_matrix)
            && (2.50..2.70).contains(&overall.rest_translation),
        "the sweep measured almost no demand at all: bounds {:.3}, skin matrix {:.3}, rest \
             translation {:.3}. A base has been loosened, or the population no longer reaches \
             the cancellations it is built to reach — either way these figures are not a \
             calibration of anything.",
        overall.bounds,
        overall.skin_matrix,
        overall.rest_translation,
    );
    let allowed = f64::from(ScaleTolerancePolicy::APPENDIX_D_V6.f32_rounding_ulps);
    assert!(
        overall.bounds < allowed
            && overall.skin_matrix < allowed
            && overall.rest_translation < allowed,
        "a correct candidate asked more of f32_rounding_ulps than the count allows: \
             bounds {:.3}, skin matrix {:.3}, rest translation {:.3} against {allowed}. \
             That is evidence about the comparison base before it is evidence about the count \
             — read docs/scale-calibration.md and the normative DESIGN.md Appendix D section D.1 \
             before raising anything.",
        overall.bounds,
        overall.skin_matrix,
        overall.rest_translation,
    );
    assert!(
        deep.rest_translation < allowed
            && deep.trajectory < allowed
            && deep.skin_matrix < allowed
            && deep.bounds < allowed,
        "deep-chain demand escaped the calibrated count: RestTranslation {:.3}, Trajectory \
             {:.3}, SkinMatrix {:.3}, Bounds {:.3}; policy allows {allowed}",
        deep.rest_translation,
        deep.trajectory,
        deep.skin_matrix,
        deep.bounds,
    );
    assert!(
        deep_by_depth.iter().all(|measured| {
            measured.rest_translation < allowed
                && measured.trajectory < allowed
                && measured.skin_matrix < allowed
                && measured.bounds < allowed
        }),
        "a declared depth escaped the calibrated count: {deep_by_depth:#?}",
    );
    assert!(
        deep.rest_translation > 0.7
            && deep.trajectory > 0.7
            && deep.skin_matrix > 0.13
            && deep.bounds > 0.14,
        "the deep-chain calibration went silent or its bases were over-inflated: \
             RestTranslation {:.3}, Trajectory {:.3}, SkinMatrix {:.3}, Bounds {:.3}",
        deep.rest_translation,
        deep.trajectory,
        deep.skin_matrix,
        deep.bounds,
    );
}
