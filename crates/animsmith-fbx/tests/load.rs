use animsmith_core::glam::{Mat4, Vec3};
use animsmith_core::measure::measure_assets;
use animsmith_core::model::{
    Property, SourceInverseBindAccessorStatus, SourceNodeLocalRest, SourceSkeletonCoverage,
};
use animsmith_core::scale::{
    ScaleCapabilityCoverage, ScaleError, ScaleOperation, ScaleRequest, plan_scale,
};
use animsmith_core::{
    DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES, DependencyClosureCoverageReasonV1,
    DependencyReferenceTargetV1, DependencyResourceRefusalReasonV1,
    DependencyResourceUnavailableReasonV1, Document, ExactFbxTimingObservationStateV1,
    ExactFbxTimingUnavailableReasonV1, FBX_KTIME_LEGACY_TICKS_PER_SECOND,
    FBX_KTIME_STANDARD_TICKS_PER_SECOND, FbxTimeModeV1, FbxTimeProtocolV1, FbxTimeSpanSelectionV1,
    InputIdentity, MeasurementContract, RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES,
    RAW_SOURCE_V1_MAX_TEXT_BYTES, SourceAxisV1, SourceChannelPropertyV1, SourceConstructKindV1,
    SourceFormatV1, SourceLoaderDispositionV1, SourceObservationStateV1, SourceProvenanceKindV1,
    SourceResourceKindV1, SourceResourceLocatorV1, SourceSetCoverageStateV1,
    SourceUnavailableReasonV1, TrackValues, validate_document_shape,
};
use animsmith_fbx::{FbxCoordinateAxis, FbxScaleDomainStatus};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A valid 1x1 PNG used as an externally referenced FBX normal map.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xA7, 0x35, 0x81, 0x84, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

const TINY_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4//8/AAX+Av6nNYGEAAAAAElFTkSuQmCC";

/// A valid, self-authored 1x1 JPEG used to prove opaque byte pass-through.
const TINY_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xDB, 0x00, 0x43, 0x01, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC0,
    0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01, 0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11,
    0x01, 0xFF, 0xC4, 0x00, 0x15, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0xFF, 0xC4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xC4,
    0x00, 0x14, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0xFF, 0xC4, 0x00, 0x14, 0x11, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xDA, 0x00, 0x0C, 0x03, 0x01,
    0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3F, 0x00, 0xA0, 0x00, 0xFF, 0xD9,
];

#[derive(Clone, Copy)]
enum NormalImage {
    Linked(&'static [u8]),
    Embedded,
    MissingWithParentDecoy,
    Unreadable,
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/rigged_triangle.fbx")
}

fn analytic_timing_fixture(global_properties: &[&str]) -> String {
    let source = std::fs::read_to_string(fixture())
        .expect("read exact-timing fixture")
        .replace("\r\n", "\n");
    let anchor = "\t\tP: \"OriginalUnitScaleFactor\", \"double\", \"Number\", \"\",1";
    let replacement = std::iter::once(anchor)
        .chain(global_properties.iter().map(|property| *property))
        .collect::<Vec<_>>()
        .join("\n");
    source.replacen(anchor, &replacement, 1)
}

fn appendix_d4_domain_names() -> Vec<&'static str> {
    let appendix = include_str!("../../../DESIGN.md")
        .split_once("### D.4 Modeled domains and preservation coverage")
        .expect("DESIGN has Appendix D.4")
        .1;
    appendix
        .lines()
        .skip_while(|line| !line.starts_with("| Domain |"))
        .skip(2)
        .take_while(|line| line.starts_with('|'))
        .map(|line| {
            line.split('|')
                .nth(1)
                .expect("D.4 row has a domain cell")
                .trim()
        })
        .collect()
}

fn assert_same_loaded_shape(left: &Document, right: &Document) {
    assert_eq!(left.source.path, right.source.path);
    assert_eq!(left.source.format, right.source.format);
    assert_eq!(left.skeleton.bones.len(), right.skeleton.bones.len());
    for (left, right) in left.skeleton.bones.iter().zip(&right.skeleton.bones) {
        assert_eq!(left.name, right.name);
        assert_eq!(left.parent, right.parent);
        assert_eq!(left.rest, right.rest);
        assert_eq!(left.inverse_bind, right.inverse_bind);
    }
    assert_eq!(left.clips.len(), right.clips.len());
    for (left, right) in left.clips.iter().zip(&right.clips) {
        assert_eq!(left.name, right.name);
        assert_eq!(left.duration_s, right.duration_s);
        assert_eq!(left.tracks.len(), right.tracks.len());
        for (left, right) in left.tracks.iter().zip(&right.tracks) {
            assert_eq!(left.bone, right.bone);
            assert_eq!(left.property, right.property);
            assert_eq!(left.interpolation, right.interpolation);
            assert_eq!(left.times, right.times);
            match (&left.values, &right.values) {
                (TrackValues::Vec3s(left), TrackValues::Vec3s(right)) => {
                    assert_eq!(left, right);
                }
                (TrackValues::Quats(left), TrackValues::Quats(right)) => {
                    assert_eq!(left, right);
                }
                _ => panic!("track value kinds differ"),
            }
        }
    }
    assert_eq!(left.assets.meshes.len(), right.assets.meshes.len());
    assert_eq!(left.assets.instances.len(), right.assets.instances.len());
    assert_eq!(left.assets.materials.len(), right.assets.materials.len());
    assert_eq!(
        left.assets.material_resources.textures.len(),
        right.assets.material_resources.textures.len()
    );
    assert_eq!(left.assets.scenes.len(), right.assets.scenes.len());
    assert_eq!(left.assets.default_scene, right.assets.default_scene);
}

fn assert_vec3_near(got: Vec3, want: Vec3) {
    assert!(
        (got - want).length() < 1e-5,
        "expected {want:?}, got {got:?}"
    );
}

fn rest_models(doc: &animsmith_core::Document) -> Vec<Mat4> {
    let mut model = vec![Mat4::IDENTITY; doc.skeleton.bones.len()];
    for (index, bone) in doc.skeleton.bones.iter().enumerate() {
        let local = bone.rest.to_mat4();
        model[index] = bone.parent.map_or(local, |parent| model[parent] * local);
    }
    model
}

#[test]
fn loads_self_authored_rigged_triangle_fixture() {
    let doc = animsmith_fbx::load(&fixture()).expect("FBX fixture loads");

    assert_eq!(doc.source.format.as_deref(), Some("fbx"));
    let bones: Vec<&str> = doc.skeleton.bones.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(bones, vec!["<fbx-root>", "root", "tri"]);
    assert_eq!(doc.skeleton.bones[1].parent, Some(0));
    assert_eq!(doc.skeleton.bones[2].parent, Some(1));

    assert_eq!(doc.clips.len(), 1);
    let clip = &doc.clips[0];
    assert_eq!(clip.name, "take");
    assert!((clip.duration_s - 1.0).abs() < 1e-6);

    let translation = clip
        .tracks
        .iter()
        .find(|t| t.bone == 1 && t.property == Property::Translation)
        .expect("root translation track");
    assert_eq!(translation.key_count(), 31);
    assert_vec3_near(translation.key_vec3(0).unwrap(), Vec3::ZERO);
    assert_vec3_near(
        translation.key_vec3(translation.key_count() - 1).unwrap(),
        Vec3::new(1.0, 0.0, 0.0),
    );

    let mesh = doc.assets.meshes.first().expect("mesh loaded");
    let instance = doc.assets.instances.first().expect("mesh instance loaded");
    assert_eq!(mesh.name, "tri");
    assert_eq!(instance.node, 2);
    assert_eq!(mesh.source_mesh_index, 0);
    assert_eq!(instance.mesh, 0);
    assert_eq!(instance.skin_joints, vec![1]);
    assert_eq!(instance.skin_ibms.len(), 1);
    assert_eq!(doc.assets.default_scene, Some(0));
    assert_eq!(doc.assets.scenes.len(), 1);
    assert_eq!(doc.assets.scenes[0].source_scene_index, 0);
    assert!(
        !doc.assets.scenes[0].roots.is_empty(),
        "FBX scene has roots"
    );

    let prim = mesh.primitives.first().expect("primitive loaded");
    assert_eq!(prim.positions.len(), 3);
    assert_eq!(prim.indices, vec![0, 1, 2]);
    assert_eq!(prim.joints, vec![[0, 0, 0, 0]; 3]);
    assert_eq!(prim.weights, vec![[1.0, 0.0, 0.0, 0.0]; 3]);

    assert_eq!(doc.assets.materials.len(), 0);
}

#[test]
fn checked_in_fixtures_publish_complete_conservative_scale_inventories() {
    for (name, take_count) in [
        ("rigged_triangle.fbx", 1),
        ("rigged_triangle_empty_take.fbx", 2),
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join(name);
        let source = animsmith_fbx::load_scale_source(&path).expect("fixture loads");
        let inventory = source.inventory();
        let domains = &inventory.domains;

        // Every Appendix D.4 row is stated explicitly. In particular,
        // normalization/rebuilding/unverifiable spans are not collapsed into
        // an absent flag merely because this small fixture lacks exotic data.
        assert_eq!(domains.rest_hierarchy, FbxScaleDomainStatus::Normalized);
        assert_eq!(domains.translation_animation, FbxScaleDomainStatus::Baked);
        assert_eq!(
            domains.rotation_and_scale_animation,
            FbxScaleDomainStatus::Baked
        );
        assert_eq!(
            domains.root_motion_and_velocity,
            FbxScaleDomainStatus::Derived
        );
        assert_eq!(domains.base_mesh_geometry, FbxScaleDomainStatus::Rebuilt);
        assert_eq!(domains.morphs, FbxScaleDomainStatus::Absent);
        assert_eq!(domains.skin_binds, FbxScaleDomainStatus::Derived);
        assert_eq!(domains.cameras_and_lights, FbxScaleDomainStatus::Absent);
        assert_eq!(
            domains.collision_and_custom_data,
            FbxScaleDomainStatus::Absent
        );
        assert_eq!(
            domains.other_vertex_and_source_data,
            FbxScaleDomainStatus::Rebuilt
        );
        assert_eq!(
            domains.out_of_contract_node_transforms,
            FbxScaleDomainStatus::Normalized
        );
        assert_eq!(
            domains.animation_targeting_matrix_nodes,
            FbxScaleDomainStatus::Baked
        );
        assert_eq!(
            domains.shared_raw_accessor_payloads,
            FbxScaleDomainStatus::Unverifiable
        );
        assert_eq!(
            domains.unreferenced_accessor_payloads,
            FbxScaleDomainStatus::Unverifiable
        );
        assert_eq!(
            domains.image_payload_aliases,
            FbxScaleDomainStatus::Unverifiable
        );
        assert_eq!(
            domains.named_rows().map(|(name, _)| name).as_slice(),
            appendix_d4_domain_names(),
            "the public FBX inventory must name every current D.4 row in table order"
        );

        assert_eq!(
            inventory.coordinate_normalization.original_up_axis,
            FbxCoordinateAxis::Unknown
        );
        assert_eq!(
            inventory.coordinate_normalization.original_unit_meters,
            0.01
        );
        assert!(inventory.coordinate_normalization.target_right_handed_y_up);
        assert_eq!(inventory.coordinate_normalization.target_unit_meters, 1.0);
        assert!(inventory.coordinate_normalization.adjust_transforms);
        assert!(inventory.animation_takes_baked);
        assert!(!inventory.authored_curve_keys_preserved);
        assert_eq!(inventory.animation_take_count, take_count);
        assert_eq!(inventory.source_animation_curve_count, 1);
        assert!(inventory.inherit_modes_compensated);
        assert_eq!(inventory.generated_geometry_helper_node_count, 0);
        assert_eq!(inventory.generated_scale_helper_node_count, 0);
        assert_eq!(inventory.compensated_inherit_node_count, 0);
        assert_eq!(inventory.generated_normal_mesh_count, 1);
        assert_eq!(inventory.missing_normal_mesh_count, 0);
        assert_eq!(inventory.skin_deformer_count, 1);
        assert_eq!(inventory.skin_cluster_count, 1);
        assert_eq!(inventory.empty_skin_deformer_count, 0);
        assert_eq!(inventory.incomplete_bind_cluster_count, 0);
        assert_eq!(inventory.bone_convenience_bind_overwrite_count, 0);
        assert!(!inventory.identity_bind_defaults_invented);
        assert_eq!(inventory.truncated_influence_vertex_count, 0);
        assert_eq!(inventory.discarded_influence_count, 0);
        assert_eq!(inventory.renormalized_influence_vertex_count, 0);
        assert_eq!(inventory.rejected_influence_count, 0);
        assert_eq!(inventory.missing_skin_influence_corner_count, 0);
        assert_eq!(inventory.non_triangle_face_count, 0);
        assert_eq!(inventory.triangulated_face_count, 0);
        assert_eq!(inventory.omitted_non_polygon_face_count, 0);
        assert_eq!(inventory.empty_mesh_definition_count, 0);
        assert!(inventory.empty_source_meshes.is_empty());
        assert_eq!(inventory.pre_weld_vertex_count, 3);
        assert_eq!(inventory.post_weld_vertex_count, 3);
        assert_eq!(inventory.multiple_skin_deformer_mesh_count, 0);
        assert_eq!(inventory.dual_quaternion_skin_count, 0);
        assert_eq!(inventory.blend_deformer_count, 0);
        assert_eq!(inventory.blend_channel_count, 0);
        assert_eq!(inventory.blend_shape_count, 0);
        assert_eq!(inventory.cache_deformer_count, 0);
        assert_eq!(inventory.unsupported_vertex_payload_mesh_count, 0);
        assert_eq!(inventory.camera_count, 0);
        assert_eq!(inventory.light_count, 0);
        assert_eq!(inventory.shared_mesh_definition_count, 0);
        assert_eq!(inventory.uninstanced_mesh_definition_count, 0);
        assert!(inventory.uninstanced_source_meshes.is_empty());
        assert_eq!(inventory.user_defined_property_count, 0);
        assert_eq!(inventory.unsupported_source_element_count, 0);
        assert_eq!(inventory.external_resource_count, 0);

        assert_eq!(
            inventory
                .source_nodes
                .iter()
                .map(|row| (row.source_index, row.ufbx_typed_id, row.ufbx_element_id))
                .collect::<Vec<_>>(),
            vec![(0, 0, 0), (1, 1, 1), (2, 2, 3)]
        );
        assert_eq!(
            inventory
                .source_meshes
                .iter()
                .map(|row| (row.source_index, row.ufbx_typed_id, row.ufbx_element_id))
                .collect::<Vec<_>>(),
            vec![(0, 0, 2)]
        );
        assert_eq!(
            inventory
                .source_skins
                .iter()
                .map(|row| (row.source_index, row.ufbx_typed_id, row.ufbx_element_id))
                .collect::<Vec<_>>(),
            vec![(0, 0, 4)]
        );

        let facts = animsmith_fbx::capability_facts(inventory);
        assert_eq!(facts.coverage, ScaleCapabilityCoverage::Complete);
        assert!(facts.unknown_source_members_present);
        assert!(facts.unsafe_accessor_layout_present);
        assert!(!facts.is_supported());
    }
}

#[test]
fn connected_stackless_animation_is_unsupported_instead_of_absent() {
    let source = std::fs::read_to_string(fixture())
        .expect("read fixture")
        .replace("\r\n", "\n")
        .replacen("\tCount: 8", "\tCount: 7", 1)
        .replacen("\tObjectType: \"AnimationStack\" { Count: 1 }\n", "", 1);
    let stack = r#"	AnimationStack: 3001, "AnimStack::take", "" {
		Properties70: {
			P: "LocalStart", "KTime", "Time", "",0
			P: "LocalStop", "KTime", "Time", "",46186158000
			P: "ReferenceStart", "KTime", "Time", "",0
			P: "ReferenceStop", "KTime", "Time", "",46186158000
		}
	}
"#;
    assert_eq!(source.matches(stack).count(), 1);
    let source = source
        .replacen(stack, "", 1)
        .replacen("\tC: \"OO\",3002,3001", "", 1);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("stackless-animation.fbx");
    std::fs::write(&path, source).expect("write analytic stackless-animation fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("stackless fixture parses");
    assert!(loaded.document().clips.is_empty());
    assert_eq!(loaded.inventory().animation_take_count, 0);
    assert_eq!(loaded.inventory().source_animation_curve_count, 1);
    assert!(!loaded.inventory().authored_curve_keys_preserved);
    assert_eq!(
        loaded.inventory().domains.translation_animation,
        FbxScaleDomainStatus::Unsupported
    );
    assert_eq!(
        loaded.inventory().domains.rotation_and_scale_animation,
        FbxScaleDomainStatus::Unsupported
    );
    assert_eq!(
        loaded.inventory().domains.root_motion_and_velocity,
        FbxScaleDomainStatus::Unsupported
    );
    assert_eq!(
        loaded.inventory().domains.animation_targeting_matrix_nodes,
        FbxScaleDomainStatus::Unsupported
    );
    let constructs = loaded.source_facts().constructs();
    assert_eq!(
        constructs.coverage().state(),
        SourceSetCoverageStateV1::Complete
    );
    assert!(constructs.rows().iter().any(|row| {
        row.kind() == SourceConstructKindV1::UnknownElement
            && row.name().as_str() == "fbx:stackless-animation"
            && row.count() > 0
            && row.disposition() == SourceLoaderDispositionV1::Unsupported
    }));
}

#[test]
fn capability_projection_maps_each_core_refusal_domain_independently() {
    type InventoryMutation = fn(&mut animsmith_fbx::FbxScaleCapabilityInventory);

    let source = animsmith_fbx::load_scale_source(&fixture()).expect("fixture loads");
    let baseline = source.inventory();
    let cases: &[(&str, InventoryMutation)] = &[
        ("morph-deformer", |inventory| {
            inventory.blend_deformer_count = 1
        }),
        ("morph-channel", |inventory| {
            inventory.blend_channel_count = 1
        }),
        ("morph-shape", |inventory| inventory.blend_shape_count = 1),
        ("camera", |inventory| inventory.camera_count = 1),
        ("light", |inventory| inventory.light_count = 1),
        ("instancing", |inventory| {
            inventory.shared_mesh_definition_count = 1
        }),
        ("uninstanced-mesh", |inventory| {
            inventory.uninstanced_mesh_definition_count = 1
        }),
        ("empty-mesh", |inventory| {
            inventory.empty_mesh_definition_count = 1
        }),
        ("extension", |inventory| {
            inventory.unsupported_source_element_count = 1
        }),
        ("extras", |inventory| {
            inventory.user_defined_property_count = 1
        }),
        ("non-triangle", |inventory| {
            inventory.non_triangle_face_count = 1
        }),
        ("vertex-payload", |inventory| {
            inventory.unsupported_vertex_payload_mesh_count = 1
        }),
        ("missing-influence", |inventory| {
            inventory.missing_skin_influence_corner_count = 1
        }),
        ("rejected-influence", |inventory| {
            inventory.rejected_influence_count = 1
        }),
        ("secondary-influence", |inventory| {
            inventory.truncated_influence_vertex_count = 1
        }),
        ("inverse-bind", |inventory| {
            inventory.incomplete_bind_cluster_count = 1
        }),
        ("external-resource", |inventory| {
            inventory.external_resource_count = 1
        }),
    ];
    for (label, mutate) in cases {
        let mut inventory = baseline.clone();
        mutate(&mut inventory);
        let facts = animsmith_fbx::capability_facts(&inventory);
        let mapped = match *label {
            "morph-deformer" | "morph-channel" | "morph-shape" => {
                facts.morphs_present && facts.morph_weights_present
            }
            "camera" => facts.cameras_present,
            "light" => facts.lights_present,
            "instancing" => facts.instancing_present,
            "extension" => facts.unregistered_extensions_present,
            "extras" => facts.extras_present,
            "non-triangle" => facts.non_triangle_primitives_present,
            "uninstanced-mesh" | "empty-mesh" | "vertex-payload" => {
                facts.unsupported_vertex_attributes_present
            }
            "missing-influence" | "rejected-influence" => {
                facts.unsupported_vertex_attributes_present
            }
            "secondary-influence" => facts.secondary_skin_influences_present,
            "inverse-bind" => facts.inverse_bind_issues_present,
            "external-resource" => facts.external_resources_present,
            _ => false,
        };
        assert!(mapped, "{label} must project to its core refusal flag");
    }
}

#[test]
fn narrow_rest_bind_gate_consumes_every_inventory_refusal_signal() {
    type InventoryMutation = fn(&mut animsmith_fbx::FbxScaleCapabilityInventory);

    let source = animsmith_fbx::load_scale_source(&fixture()).expect("fixture loads");
    let baseline = source.inventory();
    assert!(
        animsmith_fbx::rest_bind_capability_facts(baseline).is_ok(),
        "the checked-in normalized fixture is the accepted subset"
    );

    let cases: &[(&str, &str, InventoryMutation)] = &[
        (
            "rest-hierarchy-domain",
            "domain.rest_hierarchy=unsupported",
            |inventory| inventory.domains.rest_hierarchy = FbxScaleDomainStatus::Unsupported,
        ),
        (
            "translation-animation-domain",
            "domain.translation_animation=unsupported",
            |inventory| inventory.domains.translation_animation = FbxScaleDomainStatus::Unsupported,
        ),
        (
            "rotation-scale-animation-domain",
            "domain.rotation_and_scale_animation=unsupported",
            |inventory| {
                inventory.domains.rotation_and_scale_animation = FbxScaleDomainStatus::Unsupported
            },
        ),
        (
            "root-motion-domain",
            "domain.root_motion_and_velocity=unsupported",
            |inventory| {
                inventory.domains.root_motion_and_velocity = FbxScaleDomainStatus::Unsupported
            },
        ),
        (
            "base-mesh-domain",
            "domain.base_mesh_geometry=unsupported",
            |inventory| inventory.domains.base_mesh_geometry = FbxScaleDomainStatus::Unsupported,
        ),
        ("morph-domain", "domain.morphs=unsupported", |inventory| {
            inventory.domains.morphs = FbxScaleDomainStatus::Unsupported
        }),
        (
            "skin-bind-domain",
            "domain.skin_binds=unsupported",
            |inventory| inventory.domains.skin_binds = FbxScaleDomainStatus::Unsupported,
        ),
        (
            "camera-light-domain",
            "domain.cameras_and_lights=unsupported",
            |inventory| inventory.domains.cameras_and_lights = FbxScaleDomainStatus::Unsupported,
        ),
        (
            "other-source-domain",
            "domain.other_vertex_and_source_data=unsupported",
            |inventory| {
                inventory.domains.other_vertex_and_source_data = FbxScaleDomainStatus::Unsupported
            },
        ),
        (
            "out-of-contract-transform-domain",
            "domain.out_of_contract_node_transforms=unsupported",
            |inventory| {
                inventory.domains.out_of_contract_node_transforms =
                    FbxScaleDomainStatus::Unsupported
            },
        ),
        (
            "matrix-target-domain",
            "domain.animation_targeting_matrix_nodes=unsupported",
            |inventory| {
                inventory.domains.animation_targeting_matrix_nodes =
                    FbxScaleDomainStatus::Unsupported
            },
        ),
        (
            "shared-raw-span",
            "domain.shared_raw_accessor_payloads=absent (expected unverifiable)",
            |inventory| {
                inventory.domains.shared_raw_accessor_payloads = FbxScaleDomainStatus::Absent
            },
        ),
        (
            "unreferenced-raw-span",
            "domain.unreferenced_accessor_payloads=absent (expected unverifiable)",
            |inventory| {
                inventory.domains.unreferenced_accessor_payloads = FbxScaleDomainStatus::Absent
            },
        ),
        (
            "image-alias-raw-span",
            "domain.image_payload_aliases=absent (expected unverifiable)",
            |inventory| inventory.domains.image_payload_aliases = FbxScaleDomainStatus::Absent,
        ),
        (
            "custom-domain-mismatch",
            "domain.collision_and_custom_data=unsupported (expected absent)",
            |inventory| {
                inventory.domains.collision_and_custom_data = FbxScaleDomainStatus::Unsupported
            },
        ),
        (
            "target-not-y-up",
            "coordinate_normalization.target_right_handed_y_up=false",
            |inventory| inventory.coordinate_normalization.target_right_handed_y_up = false,
        ),
        (
            "target-not-meters",
            "coordinate_normalization.target_unit_meters=0.01",
            |inventory| inventory.coordinate_normalization.target_unit_meters = 0.01,
        ),
        (
            "unadjusted-transforms",
            "coordinate_normalization.adjust_transforms=false",
            |inventory| inventory.coordinate_normalization.adjust_transforms = false,
        ),
        (
            "takes-not-baked",
            "animation_takes_baked=false",
            |inventory| inventory.animation_takes_baked = false,
        ),
        (
            "authored-curves-retained",
            "authored_curve_keys_preserved=true",
            |inventory| inventory.authored_curve_keys_preserved = true,
        ),
        (
            "missing-normal",
            "missing_normal_mesh_count=1",
            |inventory| inventory.missing_normal_mesh_count = 1,
        ),
        (
            "inherited-mode-uncompensated",
            "inherit_modes_compensated=false",
            |inventory| inventory.inherit_modes_compensated = false,
        ),
        ("empty-skin", "empty_skin_deformer_count=1", |inventory| {
            inventory.empty_skin_deformer_count = 1
        }),
        (
            "convenience-bind-overwrite",
            "bone_convenience_bind_overwrite_count=1",
            |inventory| inventory.bone_convenience_bind_overwrite_count = 1,
        ),
        (
            "invented-bind-default",
            "identity_bind_defaults_invented=true",
            |inventory| inventory.identity_bind_defaults_invented = true,
        ),
        (
            "triangulated-face",
            "triangulated_face_count=1",
            |inventory| inventory.triangulated_face_count = 1,
        ),
        (
            "omitted-face",
            "omitted_non_polygon_face_count=1",
            |inventory| inventory.omitted_non_polygon_face_count = 1,
        ),
        (
            "multiple-skins",
            "multiple_skin_deformer_mesh_count=1",
            |inventory| inventory.multiple_skin_deformer_mesh_count = 1,
        ),
        (
            "dual-quaternion",
            "dual_quaternion_skin_count=1",
            |inventory| inventory.dual_quaternion_skin_count = 1,
        ),
        ("blend-deformer", "blend_deformer_count=1", |inventory| {
            inventory.blend_deformer_count = 1
        }),
        ("blend-channel", "blend_channel_count=1", |inventory| {
            inventory.blend_channel_count = 1
        }),
        ("blend-shape", "blend_shape_count=1", |inventory| {
            inventory.blend_shape_count = 1
        }),
        ("cache-deformer", "cache_deformer_count=1", |inventory| {
            inventory.cache_deformer_count = 1
        }),
        (
            "unsupported-vertex-payload",
            "unsupported_vertex_payload_mesh_count=1",
            |inventory| inventory.unsupported_vertex_payload_mesh_count = 1,
        ),
        (
            "shared-mesh",
            "shared_mesh_definition_count=1",
            |inventory| inventory.shared_mesh_definition_count = 1,
        ),
        (
            "unknown-source-element",
            concat!(
                "domain.collision_and_custom_data=absent (expected unsupported); ",
                "unsupported_source_element_count=1"
            ),
            |inventory| inventory.unsupported_source_element_count = 1,
        ),
        (
            "truncated-influence",
            "truncated_influence_vertex_count=1",
            |inventory| inventory.truncated_influence_vertex_count = 1,
        ),
        (
            "discarded-influence",
            "discarded_influence_count=1",
            |inventory| inventory.discarded_influence_count = 1,
        ),
        (
            "renormalized-influence",
            "renormalized_influence_vertex_count=1",
            |inventory| inventory.renormalized_influence_vertex_count = 1,
        ),
        (
            "rejected-influence",
            "rejected_influence_count=1",
            |inventory| inventory.rejected_influence_count = 1,
        ),
        (
            "missing-influence",
            "missing_skin_influence_corner_count=1",
            |inventory| inventory.missing_skin_influence_corner_count = 1,
        ),
        (
            "non-triangle-face",
            "non_triangle_face_count=1",
            |inventory| inventory.non_triangle_face_count = 1,
        ),
        (
            "incomplete-bind",
            "incomplete_bind_cluster_count=1",
            |inventory| inventory.incomplete_bind_cluster_count = 1,
        ),
        ("empty-mesh", "empty_mesh_definition_count=1", |inventory| {
            inventory.empty_mesh_definition_count = 1
        }),
        (
            "uninstanced-mesh",
            "uninstanced_mesh_definition_count=1",
            |inventory| inventory.uninstanced_mesh_definition_count = 1,
        ),
        ("camera", "camera_count=1", |inventory| {
            inventory.camera_count = 1
        }),
        ("light", "light_count=1", |inventory| {
            inventory.light_count = 1
        }),
        (
            "external-resource",
            "external_resource_count=1",
            |inventory| inventory.external_resource_count = 1,
        ),
    ];
    for (label, expected, mutate) in cases {
        let mut inventory = baseline.clone();
        mutate(&mut inventory);
        let error = animsmith_fbx::rest_bind_capability_facts(&inventory)
            .expect_err("mutated inventory must refuse the narrow rest/bind bridge");
        assert_eq!(
            error,
            format!("FBX rest/bind capability inventory rejected: {expected}"),
            "{label} must retain the exact one-violation diagnostic"
        );
    }

    let mut weld_mismatch = baseline.clone();
    weld_mismatch.post_weld_vertex_count = weld_mismatch.pre_weld_vertex_count + 1;
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts(&weld_mismatch).unwrap_err(),
        format!(
            "FBX rest/bind capability inventory rejected: weld_vertex_count={}!=post:{}",
            weld_mismatch.pre_weld_vertex_count, weld_mismatch.post_weld_vertex_count
        )
    );

    let mut ordered = baseline.clone();
    ordered.domains.rest_hierarchy = FbxScaleDomainStatus::Unsupported;
    ordered.coordinate_normalization.target_right_handed_y_up = false;
    ordered.blend_deformer_count = 1;
    ordered.camera_count = 1;
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts(&ordered).unwrap_err(),
        concat!(
            "FBX rest/bind capability inventory rejected: ",
            "domain.rest_hierarchy=unsupported; ",
            "coordinate_normalization.target_right_handed_y_up=false; ",
            "blend_deformer_count=1; camera_count=1"
        ),
        "multi-violation diagnostics retain authority order"
    );

    let mut custom_property = baseline.clone();
    custom_property.user_defined_property_count = 1;
    custom_property.domains.collision_and_custom_data = FbxScaleDomainStatus::Unsupported;
    let custom_facts = animsmith_fbx::rest_bind_capability_facts(&custom_property)
        .expect("user-defined properties are discarded before the scale-bearing GLB stage");
    assert!(!custom_facts.extras_present);
}

#[test]
fn polygon_triangulation_and_exact_welding_are_inventoried() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = source
        .replacen(
            "Vertices: *9 { a: 0,0,0,100,0,0,0,100,0 }",
            "Vertices: *12 { a: 0,0,0,100,0,0,100,100,0,0,100,0 }",
            1,
        )
        .replacen(
            "PolygonVertexIndex: *3 { a: 0,1,-3 }",
            "PolygonVertexIndex: *4 { a: 0,1,2,-4 }",
            1,
        )
        .replacen("Indexes: *3 { a: 0,1,2 }", "Indexes: *4 { a: 0,1,2,3 }", 1)
        .replacen("Weights: *3 { a: 1,1,1 }", "Weights: *4 { a: 1,1,1,1 }", 1);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("quad.fbx");
    std::fs::write(&path, source).expect("write analytic quad fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("quad fixture loads");
    let inventory = loaded.inventory();
    assert_eq!(inventory.non_triangle_face_count, 1);
    assert_eq!(inventory.triangulated_face_count, 1);
    assert_eq!(inventory.omitted_non_polygon_face_count, 0);
    assert_eq!(inventory.pre_weld_vertex_count, 6);
    assert_eq!(inventory.post_weld_vertex_count, 4);
    let facts = animsmith_fbx::capability_facts(inventory);
    assert!(facts.non_triangle_primitives_present);
    assert!(facts.unsupported_vertex_attributes_present);
    assert!(
        animsmith_fbx::rest_bind_capability_facts(inventory).is_err(),
        "the inventory-only boundary cannot infer why geometry changed"
    );
    let rest_bind_facts = animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect("same-parse triangulation and exact welding are scale-invariant conversions");
    assert!(!rest_bind_facts.non_triangle_primitives_present);
    assert!(!rest_bind_facts.unsupported_vertex_attributes_present);
}

#[test]
fn point_line_and_empty_meshes_make_omitted_geometry_unsupported() {
    let cases = [
        (
            "point",
            "Vertices: *3 { a: 0,0,0 }",
            "PolygonVertexIndex: *1 { a: -1 }",
            "Indexes: *1 { a: 0 }",
            "Weights: *1 { a: 1 }",
        ),
        (
            "line",
            "Vertices: *6 { a: 0,0,0,100,0,0 }",
            "PolygonVertexIndex: *2 { a: 0,-2 }",
            "Indexes: *2 { a: 0,1 }",
            "Weights: *2 { a: 1,1 }",
        ),
        ("empty", "", "", "Indexes: *0 { a: }", "Weights: *0 { a: }"),
    ];

    for (label, vertices, polygon, indexes, weights) in cases {
        let source = std::fs::read_to_string(fixture())
            .expect("read fixture")
            .replacen("Vertices: *9 { a: 0,0,0,100,0,0,0,100,0 }", vertices, 1)
            .replacen("PolygonVertexIndex: *3 { a: 0,1,-3 }", polygon, 1)
            .replacen("Indexes: *3 { a: 0,1,2 }", indexes, 1)
            .replacen("Weights: *3 { a: 1,1,1 }", weights, 1);
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(format!("{label}-face.fbx"));
        std::fs::write(&path, source).expect("write analytic omitted-face fixture");

        let loaded = animsmith_fbx::load_scale_source(&path).expect("omitted face fixture parses");
        assert_eq!(loaded.inventory().uninstanced_mesh_definition_count, 0);
        let expected_face_count = usize::from(label != "empty");
        assert_eq!(
            loaded.inventory().non_triangle_face_count,
            expected_face_count,
            "{label}"
        );
        assert_eq!(
            loaded.inventory().omitted_non_polygon_face_count,
            expected_face_count,
            "{label}"
        );
        let expected_empty_count = usize::from(label == "empty");
        assert_eq!(
            loaded.inventory().empty_mesh_definition_count,
            expected_empty_count,
            "{label}"
        );
        if label == "empty" {
            assert_eq!(
                loaded.inventory().empty_source_meshes,
                loaded.inventory().source_meshes,
                "the zero-face omission retains the complete source identity, including the ufbx element id"
            );
            assert_eq!(
                (
                    loaded.inventory().empty_source_meshes[0].source_index,
                    loaded.inventory().empty_source_meshes[0].ufbx_typed_id,
                    loaded.inventory().empty_source_meshes[0].ufbx_element_id,
                ),
                (0, 0, 2),
                "empty geometry keeps its exact stable ufbx location"
            );
        } else {
            assert!(loaded.inventory().empty_source_meshes.is_empty(), "{label}");
        }
        assert!(loaded.document().assets.meshes.is_empty(), "{label}");
        assert!(loaded.document().assets.instances.is_empty(), "{label}");
        assert_eq!(
            loaded.inventory().domains.base_mesh_geometry,
            FbxScaleDomainStatus::Unsupported,
            "{label}"
        );
        assert_eq!(
            loaded.inventory().domains.other_vertex_and_source_data,
            FbxScaleDomainStatus::Unsupported,
            "{label}"
        );
        let facts = animsmith_fbx::capability_facts(loaded.inventory());
        assert_eq!(
            facts.non_triangle_primitives_present,
            label != "empty",
            "{label}"
        );
        assert_eq!(
            facts.unsupported_vertex_attributes_present,
            label == "empty",
            "{label}"
        );
        let error = animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
            .expect_err("omitted point, line, or empty geometry remains a rest/bind blocker");
        assert!(
            error.contains(if label == "empty" {
                "empty_mesh_definition_count=1"
            } else {
                "omitted_non_polygon_face_count=1"
            }),
            "{label}: {error}"
        );
    }
}

#[test]
fn influence_truncation_renormalization_and_lossy_bone_bind_overwrite_are_inventoried() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = source.replacen(
        "ObjectType: \"Deformer\" { Count: 2 }",
        "ObjectType: \"Deformer\" { Count: 6 }",
        1,
    );
    let extra_clusters = (3..=6)
        .map(|suffix| {
            format!(
                "\tDeformer: 400{suffix}, \"SubDeformer::root_cluster_{suffix}\", \"Cluster\" {{\n\t\tVersion: 100\n\t\tIndexes: *3 {{ a: 0,1,2 }}\n\t\tWeights: *3 {{ a: 1,1,1 }}\n\t\tTransform: *16 {{ a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }}\n\t\tTransformLink: *16 {{ a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }}\n\t}}\n"
            )
        })
        .collect::<String>();
    let source = source.replacen(
        "\tAnimationStack: 3001",
        &format!("{extra_clusters}\tAnimationStack: 3001"),
        1,
    );
    let extra_connections = (3..=6)
        .map(|suffix| format!("\tC: \"OO\",400{suffix},4001\n\tC: \"OO\",1001,400{suffix}\n"))
        .collect::<String>();
    let source = source.replacen(
        "\tC: \"OO\",3002,3001",
        &format!("{extra_connections}\tC: \"OO\",3002,3001"),
        1,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("five-influences.fbx");
    std::fs::write(&path, source).expect("write analytic influence fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("influence fixture loads");
    let inventory = loaded.inventory();
    assert_eq!(inventory.skin_cluster_count, 5);
    assert_eq!(inventory.bone_convenience_bind_overwrite_count, 4);
    assert_eq!(inventory.truncated_influence_vertex_count, 3);
    assert_eq!(inventory.discarded_influence_count, 3);
    assert_eq!(inventory.renormalized_influence_vertex_count, 3);
    assert!(!inventory.identity_bind_defaults_invented);
    assert!(animsmith_fbx::capability_facts(inventory).secondary_skin_influences_present);
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded).unwrap_err(),
        "FBX rest/bind capability inventory rejected: bone_convenience_bind_overwrite_count=4",
        "bounded influence conversion is admissible without omitted mesh payload; the independently lossy repeated-bone bind remains a blocker"
    );
}

#[test]
fn non_finite_derived_cluster_bind_is_unreadable_in_every_projection() {
    let cases = [
        (
            "singular-inverse",
            "TransformLink: *16 { a: 0,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }",
            "Transform: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }",
        ),
        (
            "overflowing-product",
            "TransformLink: *16 { a: 1e-30,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }",
            "Transform: *16 { a: 1e30,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }",
        ),
    ];
    for (label, transform_link, transform) in cases {
        let source = std::fs::read_to_string(fixture())
            .expect("read fixture")
            .replacen(
                "TransformLink: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }",
                transform_link,
                1,
            )
            .replacen(
                "Transform: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }",
                transform,
                1,
            );
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(format!("{label}.fbx"));
        std::fs::write(&path, source).expect("write analytic invalid-bind fixture");

        let loaded = animsmith_fbx::load_scale_source(&path).expect("invalid bind parses");
        assert_eq!(loaded.inventory().incomplete_bind_cluster_count, 1);
        assert!(animsmith_fbx::capability_facts(loaded.inventory()).inverse_bind_issues_present);
        let accessor = &loaded.document().assets.source_skeleton.skins[0].inverse_bind_accessor;
        assert_eq!(accessor.status, SourceInverseBindAccessorStatus::Unreadable);
        assert_eq!(accessor.declared_count, Some(1));
        assert!(accessor.matrices.is_empty());
        validate_document_shape(loaded.document())
            .expect("unreadable derived bind is omitted instead of publishing non-finite matrices");
    }
}

#[test]
fn one_invalid_bind_makes_a_multi_cluster_declaration_atomically_unreadable() {
    let source = std::fs::read_to_string(fixture())
        .expect("read fixture")
        .replacen(
            "ObjectType: \"Deformer\" { Count: 2 }",
            "ObjectType: \"Deformer\" { Count: 3 }",
            1,
        );
    let invalid_cluster = r#"
	Deformer: 4003, "SubDeformer::invalid_cluster", "Cluster" {
		Version: 100
		Indexes: *3 { a: 0,1,2 }
		Weights: *3 { a: 0.5,0.5,0.5 }
		Transform: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }
		TransformLink: *16 { a: 0,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }
	}
"#;
    let source = source.replacen(
        "\tAnimationStack: 3001",
        &format!("{invalid_cluster}\tAnimationStack: 3001"),
        1,
    );
    let source = source.replacen(
        "\tC: \"OO\",3002,3001",
        "\tC: \"OO\",4003,4001\n\tC: \"OO\",1001,4003\n\tC: \"OO\",3002,3001",
        1,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("mixed-readable-binds.fbx");
    std::fs::write(&path, source).expect("write analytic mixed-bind fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("mixed bind fixture parses");
    assert_eq!(loaded.inventory().skin_cluster_count, 2);
    assert_eq!(loaded.inventory().incomplete_bind_cluster_count, 1);
    assert_eq!(
        loaded.inventory().bone_convenience_bind_overwrite_count,
        0,
        "a skipped invalid projection cannot overwrite the valid convenience bind"
    );
    assert!(
        loaded.document().skeleton.bones[1]
            .inverse_bind
            .is_some_and(|matrix| matrix.is_finite()),
        "the readable cluster is still applied to the bone convenience field"
    );
    assert_eq!(
        loaded.inventory().domains.skin_binds,
        FbxScaleDomainStatus::Unsupported
    );
    let source_skin = &loaded.document().assets.source_skeleton.skins[0];
    assert_eq!(source_skin.joint_source_node_indices, vec![1, 1]);
    assert_eq!(
        source_skin.inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Unreadable
    );
    assert_eq!(source_skin.inverse_bind_accessor.declared_count, Some(2));
    assert!(
        source_skin.inverse_bind_accessor.matrices.is_empty(),
        "an unreadable declaration cannot retain a prefix that shifts later slots"
    );

    let measured = measure_assets(loaded.document());
    assert_eq!(measured.skins.len(), 1);
    assert_eq!(
        measured.skins[0].inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Unreadable
    );
    assert!(measured.skins[0].inverse_bind_accessor.matrices.is_empty());
    assert_eq!(measured.skins[0].joints.len(), 2);
    MeasurementContract::new(BTreeMap::new(), measured)
        .expect("the measured unreadable declaration satisfies the public contract");
}

#[test]
fn zero_weight_skin_vertices_are_missing_effective_influence_evidence() {
    for (label, weights) in [("zero", "0,0,0"), ("negative", "-1,-1,-1")] {
        let source = std::fs::read_to_string(fixture())
            .expect("read fixture")
            .replacen(
                "Weights: *3 { a: 1,1,1 }",
                &format!("Weights: *3 {{ a: {weights} }}"),
                1,
            );
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(format!("{label}-weights.fbx"));
        std::fs::write(&path, source).expect("write analytic ineffective-weight fixture");

        let loaded =
            animsmith_fbx::load_scale_source(&path).expect("ineffective-weight skin parses");
        assert_eq!(
            loaded.inventory().rejected_influence_count,
            if label == "negative" { 3 } else { 0 }
        );
        assert_eq!(loaded.inventory().missing_skin_influence_corner_count, 3);
        assert_eq!(
            loaded.inventory().domains.other_vertex_and_source_data,
            FbxScaleDomainStatus::Unsupported
        );
        assert!(
            animsmith_fbx::capability_facts(loaded.inventory())
                .unsupported_vertex_attributes_present
        );
        assert_eq!(
            loaded.document().assets.meshes[0].primitives[0].weights,
            vec![[0.0; 4]; 3],
            "the convenience payload may remain zero but must not be called complete evidence"
        );
        assert!(
            animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
                .unwrap_err()
                .contains("missing_skin_influence_corner_count=3"),
            "discarded invalid influences are admissible only when effective coverage remains"
        );
    }
}

#[test]
fn mixed_positive_and_negative_influences_retain_rejection_evidence() {
    let source = std::fs::read_to_string(fixture())
        .expect("read fixture")
        .replacen(
            "ObjectType: \"Deformer\" { Count: 2 }",
            "ObjectType: \"Deformer\" { Count: 3 }",
            1,
        );
    let negative_cluster = r#"
	Deformer: 4003, "SubDeformer::negative_cluster", "Cluster" {
		Version: 100
		Indexes: *3 { a: 0,1,2 }
		Weights: *3 { a: -0.25,-0.25,-0.25 }
		Transform: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }
		TransformLink: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }
	}
"#;
    let source = source.replacen(
        "\tAnimationStack: 3001",
        &format!("{negative_cluster}\tAnimationStack: 3001"),
        1,
    );
    let source = source.replacen(
        "\tC: \"OO\",3002,3001",
        "\tC: \"OO\",4003,4001\n\tC: \"OO\",1001,4003\n\tC: \"OO\",3002,3001",
        1,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("mixed-sign-influences.fbx");
    std::fs::write(&path, source).expect("write analytic mixed-sign fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("mixed-sign fixture parses");
    assert_eq!(loaded.inventory().rejected_influence_count, 3);
    assert_eq!(loaded.inventory().missing_skin_influence_corner_count, 0);
    assert_eq!(loaded.inventory().renormalized_influence_vertex_count, 0);
    assert_eq!(
        loaded.inventory().domains.other_vertex_and_source_data,
        FbxScaleDomainStatus::Unsupported
    );
    assert!(
        animsmith_fbx::capability_facts(loaded.inventory()).unsupported_vertex_attributes_present
    );
    assert_eq!(
        loaded.document().assets.meshes[0].primitives[0].weights,
        vec![[1.0, 0.0, 0.0, 0.0]; 3],
        "the usable positive projection must not erase rejected negative source evidence"
    );
    let error = animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect_err("the duplicate-bone convenience bind remains independently unsupported");
    assert_eq!(
        error,
        "FBX rest/bind capability inventory rejected: bone_convenience_bind_overwrite_count=1",
        "rejected invalid influences are retained as evidence but are not themselves a blocker"
    );
}

#[test]
fn a_skin_cluster_without_a_bone_downgrades_the_source_projection() {
    let canonical = std::fs::read_to_string(fixture())
        .expect("read fixture")
        .replace("\r\n", "\n");
    for (label, line_ending) in [("lf", "\n"), ("crlf", "\r\n")] {
        let source = canonical.replace('\n', line_ending);
        let connection = "\tC: \"OO\",1001,4002";
        assert_eq!(source.matches(connection).count(), 1, "{label}");
        // Do not include the checkout line ending in the match: Windows may
        // materialize the checked-in fixture as CRLF while Unix uses LF.
        let source = source.replacen(connection, "", 1);
        assert!(!source.contains(connection), "{label}");
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(format!("missing-cluster-bone-{label}.fbx"));
        std::fs::write(&path, source).expect("write analytic missing-bone fixture");

        let loaded = animsmith_fbx::load_scale_source(&path).expect("missing-bone fixture parses");
        assert_eq!(
            loaded.inventory().incomplete_bind_cluster_count,
            1,
            "{label}"
        );
        assert!(
            animsmith_fbx::capability_facts(loaded.inventory()).inverse_bind_issues_present,
            "{label}"
        );
        let projection = &loaded.document().assets.source_skeleton;
        assert_eq!(
            projection.coverage,
            SourceSkeletonCoverage::Unavailable,
            "{label}"
        );
        assert!(projection.nodes.is_empty(), "{label}");
        assert!(projection.skins.is_empty(), "{label}");
        validate_document_shape(loaded.document())
            .expect("unavailable source projection leaves a valid normalized document");
        let measured = measure_assets(loaded.document());
        assert_eq!(
            measured.skeleton_source_coverage,
            SourceSkeletonCoverage::Unavailable,
            "{label}"
        );
        assert!(measured.skeleton_nodes.is_empty(), "{label}");
        assert!(measured.skins.is_empty(), "{label}");
        MeasurementContract::new(BTreeMap::new(), measured)
            .expect("downgraded source coverage is a valid public measurement");
    }
}

#[test]
fn valid_unmodeled_ufbx_typed_lists_are_counted_conservatively() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let objects = r#"
	Pose: 5001, "Pose::bind", "BindPose" {
		Type: "BindPose"
		Version: 100
		NbPoseNodes: 0
	}
	Implementation: 5002, "Implementation::shader", "" {}
	BindingTable: 5003, "BindingTable::binding", "" {}
	Cache: 5004, "Cache::detached", "" {}
"#;
    let source = source.replacen(
        "\tAnimationStack: 3001",
        &format!("{objects}\tAnimationStack: 3001"),
        1,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("unmodeled-typed-lists.fbx");
    std::fs::write(&path, source).expect("write analytic typed-list fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("typed-list fixture parses");
    assert_eq!(loaded.inventory().unsupported_source_element_count, 4);
    assert_eq!(
        loaded.inventory().domains.collision_and_custom_data,
        FbxScaleDomainStatus::Unsupported
    );
    assert_eq!(
        loaded.inventory().domains.other_vertex_and_source_data,
        FbxScaleDomainStatus::Unsupported,
        "the detached cache record is unsupported mesh/source payload"
    );
    assert!(animsmith_fbx::capability_facts(loaded.inventory()).unregistered_extensions_present);
    let error = animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect_err("unmodeled source elements remain a rest/bind refusal");
    assert_eq!(
        error,
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; count=2; ",
            "cache_files=1; incomplete_bind_poses=1)"
        ),
        "the refusal must identify every residual typed-list kind"
    );
    let constructs = loaded.source_facts().constructs();
    assert!(constructs.rows().iter().any(|row| {
        row.kind() == SourceConstructKindV1::UnknownElement
            && row.name().as_str() == "fbx:unmodeled-elements"
            && row.count() == 4
            && row.disposition() == SourceLoaderDispositionV1::Unsupported
    }));
    let resources = loaded.source_facts().resources();
    assert_eq!(
        resources.coverage().state(),
        SourceSetCoverageStateV1::Complete
    );
    let [cache] = resources.rows() else {
        panic!("the detached cache declaration is retained");
    };
    assert_eq!(cache.source_order_index(), 0);
    assert_eq!(cache.kind(), SourceResourceKindV1::Cache);
    assert_eq!(cache.locator(), &SourceResourceLocatorV1::Missing);
    assert_eq!(cache.disposition(), SourceLoaderDispositionV1::Unsupported);
}

fn add_nonbearing_node_attributes(source: &str, include_pose: bool) -> String {
    let mut objects = concat!(
        "\tNodeAttribute: 5101, \"NodeAttribute::marker\", \"FKEffector\" {}\n",
        "\tNodeAttribute: 5102, \"NodeAttribute::lod\", \"LodGroup\" {}\n",
        "\tNodeAttribute: 5103, \"NodeAttribute::stereo\", \"CameraStereo\" {}\n",
        "\tNodeAttribute: 5104, \"NodeAttribute::switcher\", \"CameraSwitcher\" {}\n",
    )
    .to_owned();
    if include_pose {
        objects.push_str(concat!(
            "\tPose: 5105, \"Pose::bind\", \"BindPose\" {\n",
            "\t\tType: \"BindPose\"\n",
            "\t\tVersion: 100\n",
            "\t\tNbPoseNodes: 0\n",
            "\t}\n",
        ));
    }
    source.replace("\r\n", "\n").replacen(
        "\tAnimationStack: 3001",
        &format!("{objects}\tAnimationStack: 3001"),
        1,
    )
}

const IDENTITY_FBX_MATRIX: &str = "1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1";

fn add_bind_pose(source: &str, root_matrix: &str, duplicate_root: bool) -> String {
    add_bind_pose_with_id(source, 5201, root_matrix, duplicate_root)
}

fn add_bind_pose_with_id(
    source: &str,
    pose_id: u64,
    root_matrix: &str,
    duplicate_root: bool,
) -> String {
    let duplicate = if duplicate_root {
        format!(
            concat!(
                "\t\tPoseNode: {{\n",
                "\t\t\tNode: 1001\n",
                "\t\t\tMatrix: *16 {{ a: {} }}\n",
                "\t\t}}\n",
            ),
            root_matrix
        )
    } else {
        String::new()
    };
    let pose = format!(
        concat!(
            "\tPose: {}, \"Pose::bind\", \"BindPose\" {{\n",
            "\t\tType: \"BindPose\"\n",
            "\t\tVersion: 100\n",
            "\t\tNbPoseNodes: {}\n",
            "\t\tPoseNode: {{\n",
            "\t\t\tNode: 1001\n",
            "\t\t\tMatrix: *16 {{ a: {} }}\n",
            "\t\t}}\n",
            "{}",
            "\t\tPoseNode: {{\n",
            "\t\t\tNode: 1002\n",
            "\t\t\tMatrix: *16 {{ a: {} }}\n",
            "\t\t}}\n",
            "\t}}\n",
        ),
        pose_id,
        if duplicate_root { 3 } else { 2 },
        root_matrix,
        duplicate,
        IDENTITY_FBX_MATRIX,
    );
    source.replace("\r\n", "\n").replacen(
        "\tAnimationStack: 3001",
        &format!("{pose}\tAnimationStack: 3001"),
        1,
    )
}

fn add_shader_and_binding(source: &str) -> String {
    let objects = concat!(
        "\tImplementation: 5301, \"Implementation::shader\", \"\" {}\n",
        "\tBindingTable: 5302, \"BindingTable::binding\", \"\" {}\n",
    );
    source.replace("\r\n", "\n").replacen(
        "\tAnimationStack: 3001",
        &format!("{objects}\tAnimationStack: 3001"),
        1,
    )
}

fn add_display_layers(source: &str) -> String {
    let display_layers = concat!(
        "\tCollectionExclusive: 5401, \"DisplayLayer::animsmith-test\", \"DisplayLayer\" {\n",
        "\t\tProperties70: {\n",
        "\t\t\tP: \"Color\", \"ColorRGB\", \"Color\", \"\",0.1,0.2,0.3\n",
        "\t\t\tP: \"Show\", \"bool\", \"\", \"\",0\n",
        "\t\t\tP: \"Freeze\", \"bool\", \"\", \"\",1\n",
        "\t\t}\n",
        "\t}\n",
        "\tCollectionExclusive: 5402, \"DisplayLayer::animsmith-test-2\", \"DisplayLayer\" {\n",
        "\t\tProperties70: {\n",
        "\t\t\tP: \"Color\", \"ColorRGB\", \"Color\", \"\",0.4,0.5,0.6\n",
        "\t\t\tP: \"Show\", \"bool\", \"\", \"\",1\n",
        "\t\t\tP: \"Freeze\", \"bool\", \"\", \"\",0\n",
        "\t\t}\n",
        "\t}\n",
    );
    source
        .replace("\r\n", "\n")
        .replacen(
            "\tAnimationStack: 3001",
            &format!("{display_layers}\tAnimationStack: 3001"),
            1,
        )
        .replacen(
            "Connections: {",
            concat!(
                "Connections: {\n",
                "\tC: \"OO\",1001,5401\n",
                "\tC: \"OO\",1001,5402",
            ),
            1,
        )
}

fn add_selection_set(source: &str) -> String {
    let objects = concat!(
        "\tCollection: 5501, \"SelectionSet::animsmith-test\", \"SelectionSet\" {}\n",
        "\tSelectionNode: 5502, \"SelectionNode::root\", \"\" {\n",
        "\t\tIsTheNodeInSet: 1\n",
        "\t}\n",
    );
    source
        .replace("\r\n", "\n")
        .replacen(
            "\tAnimationStack: 3001",
            &format!("{objects}\tAnimationStack: 3001"),
            1,
        )
        .replacen(
            "Connections: {",
            concat!(
                "Connections: {\n",
                "\tC: \"OO\",5502,5501\n",
                "\tC: \"OO\",1001,5502",
            ),
            1,
        )
}

fn add_second_root_cluster(source: &str, bind_matrix: &str) -> String {
    source
        .replacen(
            "\tAnimationStack: 3001",
            &format!(
                concat!(
                    "\tDeformer: 4003, \"SubDeformer::second_root_cluster\", \"Cluster\" {{\n",
                    "\t\tVersion: 100\n",
                    "\t\tIndexes: *3 {{ a: 0,1,2 }}\n",
                    "\t\tWeights: *3 {{ a: 1,1,1 }}\n",
                    "\t\tTransform: *16 {{ a: {} }}\n",
                    "\t\tTransformLink: *16 {{ a: {} }}\n",
                    "\t}}\n",
                    "\tAnimationStack: 3001",
                ),
                IDENTITY_FBX_MATRIX, bind_matrix,
            ),
            1,
        )
        .replacen(
            "\tC: \"OO\",1001,4002",
            concat!(
                "\tC: \"OO\",1001,4002\n",
                "\tC: \"OO\",4003,4001\n",
                "\tC: \"OO\",1001,4003",
            ),
            1,
        )
}

#[test]
fn rest_bind_admits_exact_nonbearing_node_attribute_kinds() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = add_nonbearing_node_attributes(&source, false);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("nonbearing-node-attributes.fbx");
    std::fs::write(&path, source).expect("write analytic node-attribute fixture");

    let scene = ufbx::load_memory(
        &std::fs::read(&path).expect("read analytic fixture"),
        ufbx::LoadOpts::default(),
    )
    .expect("inspect typed lists");
    assert_eq!(
        (
            scene.stereo_cameras.len(),
            scene.camera_switchers.len(),
            scene.markers.len(),
            scene.lod_groups.len(),
        ),
        (1, 1, 1, 1),
        "the analytic fixture must exercise each admitted ufbx typed list"
    );

    let loaded = animsmith_fbx::load_scale_source(&path).expect("node attributes parse");
    assert_eq!(loaded.inventory().unsupported_source_element_count, 4);
    let constructs = loaded.source_facts().constructs();
    assert!(constructs.rows().iter().any(|row| {
        row.kind() == SourceConstructKindV1::UnknownElement
            && row.name().as_str() == "fbx:unmodeled-elements"
            && row.count() == 4
    }));
    animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect("marker, LOD-group, stereo-camera, and camera-switcher rows are scale-irrelevant");
}

#[test]
fn rest_bind_admits_shader_bindings_and_a_reconciled_bind_pose() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = add_shader_and_binding(&source);
    let source = add_bind_pose(&source, IDENTITY_FBX_MATRIX, false);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("reconciled-bind-pose.fbx");
    std::fs::write(&path, source).expect("write analytic bind-pose fixture");

    let scene = ufbx::load_memory(
        &std::fs::read(&path).expect("read analytic fixture"),
        ufbx::LoadOpts::default(),
    )
    .expect("inspect typed lists");
    assert_eq!((scene.shaders.len(), scene.shader_bindings.len()), (1, 1));
    assert_eq!(scene.poses.len(), 1);
    let pose = &scene.poses[0];
    assert!(pose.is_bind_pose);
    assert_eq!(pose.bone_poses.len(), 2);

    let loaded = animsmith_fbx::load_scale_source(&path).expect("bind pose parses");
    assert_eq!(loaded.inventory().unsupported_source_element_count, 3);
    assert!(loaded.source_facts().constructs().rows().iter().any(|row| {
        row.kind() == SourceConstructKindV1::UnknownElement
            && row.name().as_str() == "fbx:unmodeled-elements"
            && row.count() == 3
    }));
    animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect("reconciled bind pose and shader metadata are scale-safe");
}

#[test]
fn rest_bind_admits_exact_display_layer_editor_metadata_count() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = add_display_layers(&source);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("display-layer.fbx");
    std::fs::write(&path, source).expect("write analytic display-layer fixture");

    let scene = ufbx::load_memory(
        &std::fs::read(&path).expect("read analytic fixture"),
        ufbx::LoadOpts::default(),
    )
    .expect("inspect display layer");
    assert_eq!(scene.display_layers.len(), 2);
    let layer = &scene.display_layers[0];
    assert_eq!(layer.nodes.len(), 1);
    assert_eq!(layer.nodes[0].element.name, "root");
    assert!(!layer.visible);
    assert!(layer.frozen);

    let loaded = animsmith_fbx::load_scale_source(&path).expect("display layer parses");
    assert_eq!(loaded.inventory().unsupported_source_element_count, 2);
    assert!(loaded.source_facts().constructs().rows().iter().any(|row| {
        row.kind() == SourceConstructKindV1::UnknownElement
            && row.name().as_str() == "fbx:unmodeled-elements"
            && row.count() == 2
    }));
    animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect("display-layer editor metadata is scale-irrelevant");
}

#[test]
fn rest_bind_keeps_selection_sets_and_nodes_unsupported() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = add_selection_set(&source);
    let source = add_display_layers(&source);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("selection-set.fbx");
    std::fs::write(&path, source).expect("write analytic selection-set fixture");

    let scene = ufbx::load_memory(
        &std::fs::read(&path).expect("read analytic fixture"),
        ufbx::LoadOpts::default(),
    )
    .expect("inspect selection set");
    assert_eq!(scene.selection_sets.len(), 1);
    assert_eq!(scene.selection_nodes.len(), 1);
    assert_eq!(scene.selection_sets[0].nodes.len(), 1);
    assert_eq!(scene.display_layers.len(), 2);

    let loaded = animsmith_fbx::load_scale_source(&path).expect("selection set parses");
    assert_eq!(loaded.inventory().unsupported_source_element_count, 4);
    assert!(loaded.source_facts().constructs().rows().iter().any(|row| {
        row.kind() == SourceConstructKindV1::UnknownElement
            && row.name().as_str() == "fbx:unmodeled-elements"
            && row.count() == 4
    }));
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded).unwrap_err(),
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; count=2; ",
            "selection_sets=1; selection_nodes=1)"
        )
    );
}

#[test]
fn rest_bind_reconciles_a_bind_pose_that_only_covers_a_non_skin_node() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = add_bind_pose(&source, IDENTITY_FBX_MATRIX, false)
        .replacen("NbPoseNodes: 2", "NbPoseNodes: 1", 1)
        .replacen(
            concat!(
                "\t\tPoseNode: {\n",
                "\t\t\tNode: 1001\n",
                "\t\t\tMatrix: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }\n",
                "\t\t}\n",
            ),
            "",
            1,
        );
    let dir = tempfile::tempdir().expect("temp dir");
    let matching_path = dir.path().join("non-skin-node-bind-pose.fbx");
    std::fs::write(&matching_path, &source).expect("write analytic bind-pose fixture");

    let loaded = animsmith_fbx::load_scale_source(&matching_path).expect("bind pose parses");
    animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect("a reconciled non-skin PoseNode is scale-safe");

    let mismatched = source.replacen(
        concat!(
            "\t\tPoseNode: {\n",
            "\t\t\tNode: 1002\n",
            "\t\t\tMatrix: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }\n",
            "\t\t}\n",
        ),
        concat!(
            "\t\tPoseNode: {\n",
            "\t\t\tNode: 1002\n",
            "\t\t\tMatrix: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,100,0,0,1 }\n",
            "\t\t}\n",
        ),
        1,
    );
    let mismatched_path = dir.path().join("mismatched-non-skin-node-bind-pose.fbx");
    std::fs::write(&mismatched_path, mismatched).expect("write mismatched bind-pose fixture");
    let loaded = animsmith_fbx::load_scale_source(&mismatched_path).expect("bind pose parses");
    assert!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
            .unwrap_err()
            .contains("mismatched_bind_poses=1")
    );
}

#[test]
fn rest_bind_refuses_a_mismatched_bind_pose() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let mismatched = "1,0,0,0,0,1,0,0,0,0,1,0,100,0,0,1";
    let source = add_bind_pose(&source, mismatched, false);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("mismatched-bind-pose.fbx");
    std::fs::write(&path, source).expect("write analytic mismatched fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("bind pose parses");
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded).unwrap_err(),
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; ",
            "count=1; mismatched_bind_poses=1)"
        )
    );
}

#[test]
fn rest_bind_checks_every_cluster_for_a_repeated_bone() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let mismatched = "1,0,0,0,0,1,0,0,0,0,1,0,100,0,0,1";
    let source = add_second_root_cluster(&source, mismatched);
    let source = add_bind_pose(&source, IDENTITY_FBX_MATRIX, false);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("later-cluster-mismatch.fbx");
    std::fs::write(&path, source).expect("write repeated-bone fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("bind pose parses");
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded).unwrap_err(),
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; ",
            "count=1; mismatched_bind_poses=1)"
        )
    );
}

#[test]
fn rest_bind_reconciles_bind_pose_with_the_fixed_scale_tolerance() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    // The fixture is authored in centimeters and the loader converts it to
    // meters, so these become 5e-7 m and 2e-6 m respectively.
    let inside = "1,0,0,0,0,1,0,0,0,0,1,0,0.00005,0,0,1";
    let inside = add_bind_pose(&source, inside, false);
    let dir = tempfile::tempdir().expect("temp dir");
    let inside_path = dir.path().join("inside-bind-pose-tolerance.fbx");
    std::fs::write(&inside_path, inside).expect("write inside-tolerance fixture");
    let loaded = animsmith_fbx::load_scale_source(&inside_path).expect("bind pose parses");
    animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect("a component inside the fixed scalar tolerance reconciles");

    let outside = "1,0,0,0,0,1,0,0,0,0,1,0,0.0002,0,0,1";
    let outside = add_bind_pose(&source, outside, false);
    let parsed = ufbx::load_memory(outside.as_bytes(), ufbx::LoadOpts::default())
        .expect("inspect outside-tolerance fixture");
    assert!(
        (parsed.poses[0].bone_poses[0].bone_to_world.m03
            - parsed.skin_clusters[0].bind_to_world.m03)
            .abs()
            > 1.0e-4,
        "fixture must retain an independently observable bind mismatch: pose={}, cluster={}",
        parsed.poses[0].bone_poses[0].bone_to_world.m03,
        parsed.skin_clusters[0].bind_to_world.m03,
    );
    let outside_path = dir.path().join("outside-bind-pose-tolerance.fbx");
    std::fs::write(&outside_path, outside).expect("write outside-tolerance fixture");
    let loaded = animsmith_fbx::load_scale_source(&outside_path).expect("bind pose parses");
    assert!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
            .unwrap_err()
            .contains("mismatched_bind_poses=1"),
        "a component outside the fixed scalar tolerance must refuse"
    );
}

#[test]
fn rest_bind_refuses_a_non_finite_bind_pose() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let non_finite = "1,0,0,0,0,1,0,0,0,0,1,0,nan,0,0,1";
    let source = add_bind_pose(&source, non_finite, false);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("non-finite-bind-pose.fbx");
    std::fs::write(&path, source).expect("write analytic non-finite fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("bind pose parses");
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded).unwrap_err(),
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; ",
            "count=1; non_finite_bind_poses=1)"
        )
    );
}

#[test]
fn rest_bind_refuses_ambiguous_bind_pose_node_coverage() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = add_bind_pose(&source, IDENTITY_FBX_MATRIX, true);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ambiguous-bind-pose.fbx");
    std::fs::write(&path, source).expect("write analytic ambiguous fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("bind pose parses");
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded).unwrap_err(),
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; ",
            "count=1; ambiguous_bind_poses=1)"
        )
    );
}

#[test]
fn rest_bind_refuses_cross_pose_node_coverage_ambiguity() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = add_bind_pose_with_id(&source, 5201, IDENTITY_FBX_MATRIX, false);
    let source = add_bind_pose_with_id(&source, 5202, IDENTITY_FBX_MATRIX, false);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("cross-pose-ambiguity.fbx");
    std::fs::write(&path, source).expect("write analytic ambiguous fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("bind poses parse");
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded).unwrap_err(),
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; ",
            "count=2; ambiguous_bind_poses=2)"
        )
    );
}

#[test]
fn rest_bind_reconciles_converted_nonidentity_bind_matrices() {
    const NONIDENTITY_FBX_MATRIX: &str = "0,1,0,0,-1,0,0,0,0,0,1,0,100,200,300,1";
    let source = std::fs::read_to_string(fixture())
        .expect("read fixture")
        .replacen(
            "P: \"UpAxis\", \"int\", \"Integer\", \"\",1",
            "P: \"UpAxis\", \"int\", \"Integer\", \"\",2",
            1,
        )
        .replacen(
            "P: \"FrontAxis\", \"int\", \"Integer\", \"\",2",
            "P: \"FrontAxis\", \"int\", \"Integer\", \"\",1",
            1,
        )
        .replacen(
            "P: \"FrontAxisSign\", \"int\", \"Integer\", \"\",1",
            "P: \"FrontAxisSign\", \"int\", \"Integer\", \"\",-1",
            1,
        )
        .replacen(
            &format!("TransformLink: *16 {{ a: {IDENTITY_FBX_MATRIX} }}"),
            &format!("TransformLink: *16 {{ a: {NONIDENTITY_FBX_MATRIX} }}"),
            1,
        );
    let source = add_bind_pose(&source, NONIDENTITY_FBX_MATRIX, false)
        .replacen("NbPoseNodes: 2", "NbPoseNodes: 1", 1)
        .replacen(
            concat!(
                "\t\tPoseNode: {\n",
                "\t\t\tNode: 1002\n",
                "\t\t\tMatrix: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }\n",
                "\t\t}\n",
            ),
            "",
            1,
        );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("converted-nonidentity-bind-pose.fbx");
    std::fs::write(&path, source).expect("write converted bind fixture");

    let parsed = ufbx::load_memory(
        &std::fs::read(&path).expect("read converted fixture"),
        ufbx::LoadOpts::default(),
    )
    .expect("inspect converted fixture");
    assert_eq!(parsed.poses.len(), 1);
    assert_eq!(parsed.poses[0].bone_poses.len(), 1);
    assert_ne!(parsed.poses[0].bone_poses[0].bone_to_world.m03, 0.0);

    let loaded = animsmith_fbx::load_scale_source(&path).expect("bind pose parses");
    animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect("converted nonidentity pose and cluster matrices reconcile");
}

#[test]
fn rest_bind_refuses_partial_bind_pose_skin_coverage() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = source
        .replace("Weights: *3 { a: 1,1,1 }", "Weights: *3 { a: 0.5,0.5,0.5 }")
        .replacen(
            "\tAnimationStack: 3001",
            concat!(
                "\tDeformer: 4003, \"SubDeformer::tri_cluster\", \"Cluster\" {\n",
                "\t\tVersion: 100\n",
                "\t\tIndexes: *3 { a: 0,1,2 }\n",
                "\t\tWeights: *3 { a: 0.5,0.5,0.5 }\n",
                "\t\tTransform: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }\n",
                "\t\tTransformLink: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }\n",
                "\t}\n",
                "\tAnimationStack: 3001",
            ),
            1,
        )
        .replacen(
            "\tC: \"OO\",1001,4002",
            concat!(
                "\tC: \"OO\",1001,4002\n",
                "\tC: \"OO\",4003,4001\n",
                "\tC: \"OO\",1002,4003",
            ),
            1,
        );
    let source = add_bind_pose(&source, IDENTITY_FBX_MATRIX, false)
        .replacen("NbPoseNodes: 2", "NbPoseNodes: 1", 1)
        .replacen(
            concat!(
                "\t\tPoseNode: {\n",
                "\t\t\tNode: 1002\n",
                "\t\t\tMatrix: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }\n",
                "\t\t}\n",
            ),
            "",
            1,
        );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("partial-bind-pose.fbx");
    std::fs::write(&path, source).expect("write analytic partial fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("bind pose parses");
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded).unwrap_err(),
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; ",
            "count=1; incomplete_bind_poses=1)"
        )
    );
}

#[test]
fn rest_bind_keeps_non_bind_pose_kinds_unsupported() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = add_bind_pose(&source, IDENTITY_FBX_MATRIX, false)
        .replacen(
            "\"Pose::bind\", \"BindPose\"",
            "\"Pose::rest\", \"RestPose\"",
            1,
        )
        .replacen("Type: \"BindPose\"", "Type: \"RestPose\"", 1);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("rest-pose.fbx");
    std::fs::write(&path, source).expect("write analytic rest-pose fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("rest pose parses");
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded).unwrap_err(),
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; ",
            "count=1; non_bind_poses=1)"
        )
    );
}

#[test]
fn admitted_node_attributes_do_not_hide_a_residual_pose() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let source = add_nonbearing_node_attributes(&source, true);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("mixed-node-attributes-and-pose.fbx");
    std::fs::write(&path, source).expect("write analytic mixed fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("mixed fixture parses");
    assert_eq!(loaded.inventory().unsupported_source_element_count, 5);
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&loaded).unwrap_err(),
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; ",
            "count=1; incomplete_bind_poses=1)"
        )
    );
}

#[test]
fn every_morph_typed_list_independently_marks_the_domain_present() {
    let cases = [
        (
            "deformer",
            "ObjectType: \"Deformer\" { Count: 2 }",
            "ObjectType: \"Deformer\" { Count: 3 }",
            "\tDeformer: 5001, \"Deformer::blend\", \"BlendShape\" { Version: 100 }\n",
            [1, 0, 0],
        ),
        (
            "channel",
            "ObjectType: \"Deformer\" { Count: 2 }",
            "ObjectType: \"Deformer\" { Count: 3 }",
            "\tDeformer: 5001, \"SubDeformer::channel\", \"BlendShapeChannel\" { Version: 100 DeformPercent: 0 FullWeights: *1 { a: 100 } }\n",
            [0, 1, 0],
        ),
        (
            "shape",
            "ObjectType: \"Geometry\" { Count: 1 }",
            "ObjectType: \"Geometry\" { Count: 2 }",
            "\tGeometry: 5001, \"Geometry::shape\", \"Shape\" { Version: 100 Indexes: *1 { a: 0 } Vertices: *3 { a: 0,0,0 } Normals: *3 { a: 0,1,0 } }\n",
            [0, 0, 1],
        ),
    ];
    for (label, definition, replacement, object, expected) in cases {
        let source = std::fs::read_to_string(fixture())
            .expect("read fixture")
            .replacen(definition, replacement, 1)
            .replacen(
                "\tAnimationStack: 3001",
                &format!("{object}\tAnimationStack: 3001"),
                1,
            );
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(format!("detached-morph-{label}.fbx"));
        std::fs::write(&path, source).expect("write analytic morph fixture");

        let loaded = animsmith_fbx::load_scale_source(&path).expect("morph fixture parses");
        assert_eq!(
            [
                loaded.inventory().blend_deformer_count,
                loaded.inventory().blend_channel_count,
                loaded.inventory().blend_shape_count,
            ],
            expected,
            "{label} must retain its own typed-list count"
        );
        assert_eq!(
            loaded.inventory().domains.morphs,
            FbxScaleDomainStatus::Unsupported
        );
        let facts = animsmith_fbx::capability_facts(loaded.inventory());
        assert!(facts.morphs_present, "{label}");
        assert!(facts.morph_weights_present, "{label}");
    }
}

#[test]
fn unsupported_vertex_payload_changes_other_source_data_from_rebuilt_to_unsupported() {
    let source = std::fs::read_to_string(fixture()).expect("read fixture");
    let colors = r#"
		LayerElementColor: 0 {
			Version: 101
			Name: "color"
			MappingInformationType: "ByPolygonVertex"
			ReferenceInformationType: "Direct"
			Colors: *12 { a: 1,0,0,1,0,1,0,1,0,0,1,1 }
		}
"#;
    let source = source.replacen(
        "\t\tPolygonVertexIndex: *3 { a: 0,1,-3 }",
        &format!("\t\tPolygonVertexIndex: *3 {{ a: 0,1,-3 }}{colors}"),
        1,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("vertex-colors.fbx");
    std::fs::write(&path, source).expect("write analytic vertex-color fixture");

    let parsed = ufbx::load_memory(
        &std::fs::read(&path).expect("read analytic vertex-color fixture"),
        ufbx::LoadOpts::default(),
    )
    .expect("inspect vertex-color fields");
    assert!(parsed.meshes[0].vertex_color.exists);
    assert_eq!(parsed.meshes[0].color_sets.len(), 1);

    let loaded = animsmith_fbx::load_scale_source(&path).expect("vertex-color fixture parses");
    assert_eq!(loaded.inventory().unsupported_vertex_payload_mesh_count, 1);
    assert_eq!(
        loaded.inventory().domains.other_vertex_and_source_data,
        FbxScaleDomainStatus::Unsupported
    );
    assert!(
        animsmith_fbx::capability_facts(loaded.inventory()).unsupported_vertex_attributes_present
    );
    assert!(
        animsmith_fbx::rest_bind_capability_facts(loaded.inventory()).is_err(),
        "the inventory-only boundary remains conservative"
    );
    let rest_bind_facts = animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect("same-parse vertex colors are scale-invariant conversion fidelity");
    assert!(!rest_bind_facts.unsupported_vertex_attributes_present);
}

#[test]
fn authored_face_and_edge_payloads_are_independently_unsupported() {
    let cases = [
        (
            "face-smoothing",
            r#"
		LayerElementSmoothing: 0 {
			Version: 102
			Name: "smoothing"
			MappingInformationType: "ByPolygon"
			ReferenceInformationType: "Direct"
			Smoothing: *1 { a: 1 }
		}
"#,
        ),
        (
            "face-hole",
            r#"
		LayerElementHole: 0 {
			Version: 100
			Name: "hole"
			MappingInformationType: "ByPolygon"
			ReferenceInformationType: "Direct"
			Hole: *1 { a: 1 }
		}
"#,
        ),
        (
            "edge-visibility",
            r#"
		Edges: *3 { a: 0,1,2 }
		LayerElementVisibility: 0 {
			Version: 100
			Name: "edge-visibility"
			MappingInformationType: "ByEdge"
			ReferenceInformationType: "Direct"
			Visibility: *3 { a: 1,0,1 }
		}
"#,
        ),
        (
            "edge-smoothing",
            r#"
		Edges: *3 { a: 0,1,2 }
		LayerElementSmoothing: 0 {
			Version: 102
			Name: "edge-smoothing"
			MappingInformationType: "ByEdge"
			ReferenceInformationType: "Direct"
			Smoothing: *3 { a: 1,0,1 }
		}
"#,
        ),
        (
            "face-group",
            r#"
		LayerElementPolygonGroup: 0 {
			Version: 100
			Name: "group"
			MappingInformationType: "ByPolygon"
			ReferenceInformationType: "Direct"
			PolygonGroup: *1 { a: 7 }
		}
"#,
        ),
        (
            "edge-and-vertex-crease",
            r#"
		Edges: *3 { a: 0,1,2 }
		LayerElementEdgeCrease: 0 {
			Version: 100
			Name: "edge-crease"
			MappingInformationType: "ByEdge"
			ReferenceInformationType: "Direct"
			EdgeCrease: *3 { a: 0.25,0.5,0.75 }
		}
		LayerElementVertexCrease: 0 {
			Version: 100
			Name: "vertex-crease"
			MappingInformationType: "ByVertex"
			ReferenceInformationType: "Direct"
			VertexCrease: *3 { a: 0.25,0.5,0.75 }
		}
"#,
        ),
        (
            "tangent-basis",
            r#"
		LayerElementUV: 0 {
			Version: 101
			Name: "uv"
			MappingInformationType: "ByPolygonVertex"
			ReferenceInformationType: "Direct"
			UV: *6 { a: 0,0,1,0,0,1 }
		}
		LayerElementTangent: 0 {
			Version: 101
			Name: "tangent"
			MappingInformationType: "ByPolygonVertex"
			ReferenceInformationType: "Direct"
			Tangents: *9 { a: 1,0,0,1,0,0,1,0,0 }
		}
		LayerElementBinormal: 0 {
			Version: 101
			Name: "bitangent"
			MappingInformationType: "ByPolygonVertex"
			ReferenceInformationType: "Direct"
			Binormals: *9 { a: 0,1,0,0,1,0,0,1,0 }
		}
		Layer: 0 {
			Version: 100
			LayerElement: { Type: "LayerElementUV" TypedIndex: 0 }
			LayerElement: { Type: "LayerElementTangent" TypedIndex: 0 }
			LayerElement: { Type: "LayerElementBinormal" TypedIndex: 0 }
		}
"#,
        ),
        (
            "extra-uv-set",
            r#"
		LayerElementUV: 0 {
			Version: 101
			Name: "uv0"
			MappingInformationType: "ByPolygonVertex"
			ReferenceInformationType: "Direct"
			UV: *6 { a: 0,0,1,0,0,1 }
		}
		LayerElementUV: 1 {
			Version: 101
			Name: "uv1"
			MappingInformationType: "ByPolygonVertex"
			ReferenceInformationType: "Direct"
			UV: *6 { a: 0.1,0.1,0.9,0.1,0.1,0.9 }
		}
"#,
        ),
        (
            "subdivision-preview",
            r#"
		PreviewDivisionLevels: 1
"#,
        ),
        (
            "subdivision-render",
            r#"
		RenderDivisionLevels: 2
"#,
        ),
        (
            "subdivision-display",
            r#"
		Smoothness: 1
"#,
        ),
        (
            "subdivision-boundary",
            r#"
		BoundaryRule: 1
"#,
        ),
    ];

    for (label, payload) in cases {
        let source = std::fs::read_to_string(fixture())
            .expect("read fixture")
            .replacen(
                "\t\tPolygonVertexIndex: *3 { a: 0,1,-3 }",
                &format!("\t\tPolygonVertexIndex: *3 {{ a: 0,1,-3 }}{payload}"),
                1,
            );
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(format!("{label}.fbx"));
        std::fs::write(&path, source).expect("write analytic face/edge fixture");

        let parsed = ufbx::load_memory(
            &std::fs::read(&path).expect("read analytic face/edge fixture"),
            ufbx::LoadOpts::default(),
        )
        .expect("inspect the independently authored ufbx mesh field");
        let mesh = &parsed.meshes[0];
        match label {
            "face-smoothing" => assert!(!mesh.face_smoothing.is_empty()),
            "face-hole" => assert!(!mesh.face_hole.is_empty()),
            "edge-visibility" => assert!(!mesh.edge_visibility.is_empty()),
            "edge-smoothing" => assert!(!mesh.edge_smoothing.is_empty()),
            "face-group" => {
                assert!(!mesh.face_group.is_empty());
                assert!(!mesh.face_groups.is_empty());
            }
            "edge-and-vertex-crease" => {
                assert!(!mesh.edge_crease.is_empty());
                assert!(mesh.vertex_crease.exists);
            }
            "tangent-basis" => {
                assert!(mesh.vertex_tangent.exists);
                assert!(mesh.vertex_bitangent.exists);
            }
            "extra-uv-set" => assert!(mesh.uv_sets.len() > 1),
            "subdivision-preview" => assert!(mesh.subdivision_preview_levels > 0),
            "subdivision-render" => assert!(mesh.subdivision_render_levels > 0),
            "subdivision-display" => assert!(!matches!(
                mesh.subdivision_display_mode,
                ufbx::SubdivisionDisplayMode::Disabled
            )),
            "subdivision-boundary" => assert!(!matches!(
                mesh.subdivision_boundary,
                ufbx::SubdivisionBoundary::Default
            )),
            _ => unreachable!("all analytic member classes are named above"),
        }

        let loaded = animsmith_fbx::load_scale_source(&path).expect("face/edge fixture parses");
        assert_eq!(
            loaded.inventory().unsupported_vertex_payload_mesh_count,
            1,
            "{label} is authored payload omitted by triangle extraction"
        );
        assert_eq!(
            loaded.inventory().domains.other_vertex_and_source_data,
            FbxScaleDomainStatus::Unsupported,
            "{label} cannot remain rebuilt"
        );
        assert!(
            animsmith_fbx::capability_facts(loaded.inventory())
                .unsupported_vertex_attributes_present,
            "{label} must reach the format-neutral refusal"
        );
        let rest_bind_facts = animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
            .expect("same-parse face and edge payload is scale-invariant conversion fidelity");
        assert!(
            !rest_bind_facts.unsupported_vertex_attributes_present,
            "{label}"
        );
    }
}

#[test]
fn a_detached_source_mesh_definition_is_not_called_rebuilt() {
    let source = std::fs::read_to_string(fixture())
        .expect("read fixture")
        .replacen(
            "ObjectType: \"Geometry\" { Count: 1 }",
            "ObjectType: \"Geometry\" { Count: 2 }",
            1,
        );
    let detached = r#"
	Geometry: 5001, "Geometry::detached", "Mesh" {
		Vertices: *9 { a: 0,0,0,100,0,0,0,100,0 }
		PolygonVertexIndex: *3 { a: 0,1,-3 }
	}
"#;
    let source = source.replacen(
        "\tAnimationStack: 3001",
        &format!("{detached}\tAnimationStack: 3001"),
        1,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("detached-mesh.fbx");
    std::fs::write(&path, source).expect("write analytic detached-mesh fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("detached mesh fixture parses");
    assert_eq!(loaded.inventory().source_meshes.len(), 2);
    assert_eq!(loaded.document().assets.meshes.len(), 1);
    assert_eq!(loaded.inventory().uninstanced_mesh_definition_count, 1);
    assert_eq!(
        loaded
            .inventory()
            .uninstanced_source_meshes
            .iter()
            .map(|row| (row.source_index, row.ufbx_typed_id, row.ufbx_element_id))
            .collect::<Vec<_>>(),
        vec![(1, 1, 6)],
        "the unsupported definition retains its exact stable source location"
    );
    assert_eq!(
        loaded.inventory().domains.base_mesh_geometry,
        FbxScaleDomainStatus::Unsupported
    );
    assert_eq!(
        loaded.inventory().domains.other_vertex_and_source_data,
        FbxScaleDomainStatus::Unsupported
    );
    assert!(
        animsmith_fbx::capability_facts(loaded.inventory()).unsupported_vertex_attributes_present
    );
}

#[test]
fn a_skipped_source_mesh_does_not_renumber_a_retained_mesh_attachment() {
    let source = std::fs::read_to_string(fixture())
        .expect("read fixture")
        .replacen("\tCount: 8", "\tCount: 10", 1)
        .replacen(
            "ObjectType: \"Model\" { Count: 2 }",
            "ObjectType: \"Model\" { Count: 3 }",
            1,
        )
        .replacen(
            "ObjectType: \"Geometry\" { Count: 1 }",
            "ObjectType: \"Geometry\" { Count: 2 }",
            1,
        );
    let omitted_point = r#"
	Geometry: 1500, "Geometry::point", "Mesh" {
		Vertices: *3 { a: 0,0,0 }
		PolygonVertexIndex: *1 { a: -1 }
	}
	Model: 1501, "Model::point", "Mesh" {
		Version: 232
		Properties70: {
			P: "Lcl Translation", "Lcl Translation", "", "A",0,0,0
			P: "Lcl Rotation", "Lcl Rotation", "", "A",0,0,0
			P: "Lcl Scaling", "Lcl Scaling", "", "A",1,1,1
		}
	}
"#;
    let source = source.replacen(
        "\tGeometry: 2001, \"Geometry::tri\", \"Mesh\" {",
        &format!("{omitted_point}\tGeometry: 2001, \"Geometry::tri\", \"Mesh\" {{"),
        1,
    );
    let source = source.replacen(
        "\tC: \"OO\",1002,1001",
        "\tC: \"OO\",1501,1001\n\tC: \"OO\",1500,1501\n\tC: \"OO\",1002,1001",
        1,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("skipped-before-retained.fbx");
    std::fs::write(&path, source).expect("write analytic source-identity fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("source-identity fixture parses");
    assert_eq!(loaded.inventory().source_meshes.len(), 2);
    assert_eq!(loaded.inventory().uninstanced_mesh_definition_count, 0);
    assert_eq!(loaded.inventory().omitted_non_polygon_face_count, 1);
    assert_eq!(loaded.document().assets.meshes.len(), 1);
    assert_eq!(loaded.document().assets.instances.len(), 1);
    assert_eq!(loaded.document().assets.meshes[0].source_mesh_index, 1);
    assert_eq!(loaded.document().assets.instances[0].mesh, 0);
    let attachment = &loaded.document().assets.source_skeleton.skins[0].attachments[0];
    assert_eq!(attachment.source_mesh_index, Some(1));
    assert_eq!(
        loaded.document().assets.meshes[0].source_mesh_index,
        attachment
            .source_mesh_index
            .expect("attachment names a mesh"),
        "the compact output index must not replace the stable ufbx join key"
    );
}

#[test]
fn shared_source_geometry_is_one_definition_with_two_instances() {
    let source = std::fs::read_to_string(fixture())
        .expect("read fixture")
        .replacen("\tCount: 8", "\tCount: 9", 1)
        .replacen(
            "ObjectType: \"Model\" { Count: 2 }",
            "ObjectType: \"Model\" { Count: 3 }",
            1,
        );
    let second_instance = r#"
	Model: 1501, "Model::tri-copy", "Mesh" {
		Version: 232
		Properties70: {
			P: "Lcl Translation", "Lcl Translation", "", "A",2,0,0
			P: "Lcl Rotation", "Lcl Rotation", "", "A",0,0,0
			P: "Lcl Scaling", "Lcl Scaling", "", "A",1,1,1
		}
	}
"#;
    let source = source.replacen(
        "\tDeformer: 4001, \"Deformer::skin\", \"Skin\" {",
        &format!("{second_instance}\tDeformer: 4001, \"Deformer::skin\", \"Skin\" {{"),
        1,
    );
    let source = source.replacen(
        "\tC: \"OO\",2001,1002",
        "\tC: \"OO\",2001,1002\n\tC: \"OO\",1501,1001\n\tC: \"OO\",2001,1501",
        1,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("shared-source-geometry.fbx");
    std::fs::write(&path, source).expect("write analytic shared-geometry fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("shared geometry fixture parses");
    let inventory = loaded.inventory();
    assert_eq!(inventory.source_meshes.len(), 1);
    assert_eq!(inventory.shared_mesh_definition_count, 1);
    assert!(animsmith_fbx::capability_facts(inventory).instancing_present);

    let document = loaded.document();
    validate_document_shape(document).expect("shared normalized document is structurally valid");
    assert_eq!(document.assets.meshes.len(), 1);
    assert_eq!(document.assets.meshes[0].source_mesh_index, 0);
    assert_eq!(document.assets.instances.len(), 2);
    assert_eq!(
        document
            .assets
            .instances
            .iter()
            .map(|instance| (instance.source_node_index, instance.node, instance.mesh))
            .collect::<Vec<_>>(),
        vec![(2, 2, 0), (3, 3, 0)]
    );

    let attachments = &document.assets.source_skeleton.skins[0].attachments;
    assert_eq!(
        attachments
            .iter()
            .map(|attachment| { (attachment.source_node_index, attachment.source_mesh_index,) })
            .collect::<Vec<_>>(),
        vec![(2, Some(0)), (3, Some(0))]
    );
    assert!(attachments.iter().all(|attachment| {
        attachment.source_mesh_index == Some(document.assets.meshes[0].source_mesh_index)
    }));

    let measured = measure_assets(document);
    assert_eq!(measured.mesh_definitions.len(), 1);
    assert_eq!(measured.node_instances.len(), 2);
    MeasurementContract::new(BTreeMap::new(), measured)
        .expect("shared source geometry satisfies the public measurement contract");
}

#[test]
fn complete_source_projection_retains_normalized_identity_and_derived_binds() {
    let source = animsmith_fbx::load_scale_source(&fixture()).expect("fixture loads");
    let document = source.document();
    validate_document_shape(document).expect("complete FBX projection is structurally valid");
    let projection = &document.assets.source_skeleton;
    assert_eq!(projection.coverage, SourceSkeletonCoverage::Complete);
    assert_eq!(projection.nodes.len(), 3);
    assert_eq!(
        projection
            .nodes
            .iter()
            .map(|node| {
                (
                    node.source_node_index,
                    node.name.as_deref(),
                    node.parent_source_node_index,
                    node.scene_root_indices.as_slice(),
                    node.bone,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (0, None, None, &[0][..], Some(0)),
            (1, Some("root"), Some(0), &[][..], Some(1)),
            (2, Some("tri"), Some(1), &[][..], Some(2)),
        ]
    );
    let SourceNodeLocalRest::Trs { scale, .. } = &projection.nodes[1].local_rest else {
        panic!("ufbx normalized projection uses TRS");
    };
    assert_eq!(*scale, Vec3::splat(0.01));
    assert_eq!(*scale, document.skeleton.bones[1].rest.scale);

    assert_eq!(projection.skins.len(), 1);
    let skin = &projection.skins[0];
    assert_eq!(skin.source_skin_index, 0);
    assert_eq!(skin.name.as_deref(), Some("skin"));
    assert_eq!(skin.skeleton_root_source_node_index, None);
    assert_eq!(skin.joint_source_node_indices, vec![1]);
    assert_eq!(
        skin.inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Available
    );
    assert_eq!(skin.inverse_bind_accessor.declared_count, Some(1));
    assert_eq!(skin.inverse_bind_accessor.matrices, vec![Mat4::IDENTITY]);
    assert_eq!(skin.attachments.len(), 1);
    assert_eq!(skin.attachments[0].source_node_index, 2);
    assert_eq!(skin.attachments[0].source_mesh_index, Some(0));
}

#[test]
fn generic_fbx_scale_facts_refuse_but_the_narrow_rest_bind_projection_is_inventory_gated() {
    let source = animsmith_fbx::load_scale_source(&fixture()).expect("fixture loads");
    let facts = animsmith_fbx::capability_facts(source.inventory());
    assert_eq!(
        animsmith_fbx::capability_facts_for_source(&source),
        facts,
        "clean shared facts preserve the existing conservative projection"
    );
    for operation in [
        ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
        ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
    ] {
        assert_eq!(
            plan_scale(&ScaleRequest {
                operation,
                document: source.document(),
                capability: &facts,
            })
            .unwrap_err(),
            ScaleError::IncompleteCapability
        );
    }

    let rest_bind_facts = animsmith_fbx::rest_bind_capability_facts(source.inventory())
        .expect("self-authored fixture has the complete narrow rest/bind inventory");
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(&source)
            .expect("clean shared facts admit the same narrow subset"),
        rest_bind_facts
    );
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
        document: source.document(),
        capability: &rest_bind_facts,
    })
    .expect("narrow projection admits the complete normalized source");

    let mut unsupported = source.inventory().clone();
    unsupported.domains.morphs = FbxScaleDomainStatus::Unsupported;
    assert!(
        animsmith_fbx::rest_bind_capability_facts(&unsupported).is_err(),
        "a present semantic domain without complete representation remains fail-closed"
    );

    // Source identity is a separate gate: even a fabricated supported
    // capability projection cannot license rest/bind after the sidecar is
    // removed. This prevents either "complete" bit from standing in for the
    // other inventory.
    let mut without_projection = source.document().clone();
    without_projection.assets.source_skeleton.coverage = SourceSkeletonCoverage::Unavailable;
    let mut fabricated = animsmith_core::scale::ScaleCapabilityFacts::default();
    fabricated.coverage = ScaleCapabilityCoverage::Complete;
    assert_eq!(
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 1,
                expected_factor: 0.01,
            },
            document: &without_projection,
            capability: &fabricated,
        })
        .unwrap_err(),
        ScaleError::IncompleteSourceSkeleton
    );
}

#[test]
fn load_path_and_captured_bytes_are_equivalent() {
    let path = fixture();
    let bytes = std::fs::read(&path).expect("capture fixture");

    let from_path = animsmith_fbx::load_scale_source(&path).expect("fixture loads by path");
    let from_bytes =
        animsmith_fbx::load_scale_source_bytes(&path, &bytes).expect("fixture loads by bytes");

    assert_eq!(from_path.inventory(), from_bytes.inventory());
    assert_same_loaded_shape(from_path.document(), from_bytes.document());
}

#[test]
fn source_facts_path_and_captured_bytes_bind_the_same_exact_input() {
    let path = fixture();
    let bytes = std::fs::read(&path).expect("capture fixture");

    let from_path = animsmith_fbx::load_source(&path).expect("source loads by path");
    let from_bytes =
        animsmith_fbx::load_source_bytes(&path, &bytes).expect("source loads by bytes");
    assert_same_loaded_shape(from_path.document(), from_bytes.document());

    let path_facts = from_path.source_facts();
    let bytes_facts = from_bytes.source_facts();
    assert_eq!(path_facts.format(), SourceFormatV1::Fbx);
    assert_eq!(
        path_facts.primary_identity(),
        &InputIdentity::from_bytes(&bytes)
    );
    assert_eq!(
        path_facts.primary_identity(),
        bytes_facts.primary_identity()
    );
    assert_eq!(path_facts.linear_unit(), bytes_facts.linear_unit());
    assert_eq!(
        path_facts.coordinate_basis(),
        bytes_facts.coordinate_basis()
    );
    assert_eq!(
        path_facts.frames_per_second(),
        bytes_facts.frames_per_second()
    );
    assert_eq!(path_facts.clips(), bytes_facts.clips());
    assert_eq!(path_facts.constructs(), bytes_facts.constructs());
    assert_eq!(path_facts.resources(), bytes_facts.resources());
    for coverage in [
        path_facts.clips().coverage(),
        path_facts.constructs().coverage(),
        path_facts.resources().coverage(),
    ] {
        assert_eq!(coverage.state(), SourceSetCoverageStateV1::Complete);
    }
}

#[test]
fn exact_fbx_timing_retains_absent_declarations_and_legacy_ktime_ticks() {
    let loaded = animsmith_fbx::load_source(&fixture()).expect("fixture loads");
    let timing = loaded
        .exact_fbx_timing()
        .expect("FBX load retains exact timing evidence");

    let ExactFbxTimingObservationStateV1::Observed(basis) = timing.ktime_basis().state() else {
        panic!("exact KTime basis");
    };
    assert_eq!(basis.ticks_per_second(), FBX_KTIME_LEGACY_TICKS_PER_SECOND);
    assert_eq!(
        timing.declared_time_mode().state(),
        &ExactFbxTimingObservationStateV1::ProvenAbsent
    );
    assert_eq!(
        timing.effective_time_mode().state(),
        &ExactFbxTimingObservationStateV1::Observed(FbxTimeModeV1::Fps24)
    );
    assert_eq!(
        timing.declared_time_protocol().state(),
        &ExactFbxTimingObservationStateV1::ProvenAbsent
    );
    assert_eq!(
        timing.declared_custom_frame_rate().state(),
        &ExactFbxTimingObservationStateV1::ProvenAbsent
    );
    assert_eq!(
        timing.effective_time_protocol().state(),
        &ExactFbxTimingObservationStateV1::Observed(FbxTimeProtocolV1::Default)
    );
    assert_eq!(
        timing
            .frame_period()
            .provenance()
            .expect("parser fallback provenance")
            .kind(),
        SourceProvenanceKindV1::ParserProjected
    );

    let ExactFbxTimingObservationStateV1::Observed(period) = timing.frame_period().state() else {
        panic!("24 fps legacy period");
    };
    assert_eq!(
        period.ticks_per_frame(),
        FBX_KTIME_LEGACY_TICKS_PER_SECOND / 24
    );
    let [stack] = timing.stacks() else {
        panic!("one exact stack row");
    };
    let ExactFbxTimingObservationStateV1::Observed(range) = stack.source_tick_range().state()
    else {
        panic!("exact source tick range");
    };
    assert_eq!(range.selection(), FbxTimeSpanSelectionV1::Local);
    assert_eq!(range.begin_ticks(), 0);
    assert_eq!(range.end_ticks(), FBX_KTIME_LEGACY_TICKS_PER_SECOND);
    assert!(period.is_whole_frame(range.end_ticks()));
}

#[test]
fn exact_fbx_timing_uses_standard_basis_and_absolute_signed_end_coordinate() {
    let period = 4_708_704i64;
    for (suffix, end_ticks, whole) in [
        ("minus-one", period - 1, false),
        ("whole", period, true),
        ("plus-one", period + 1, false),
    ] {
        let source = analytic_timing_fixture(&[
            "\t\tP: \"TimeMode\", \"enum\", \"\", \"\",8",
            "\t\tP: \"TimeProtocol\", \"enum\", \"\", \"\",0",
        ])
        .replacen("FBXHeaderVersion: 1003", "FBXHeaderVersion: 1004", 1)
        .replacen(
            "\tCreator: \"animsmith self-authored test fixture\"",
            concat!(
                "\tCreator: \"animsmith self-authored test fixture\"\n",
                "\tOtherFlags: {\n",
                "\t\tTCDefinition: 0\n",
                "\t}"
            ),
            1,
        )
        .replacen(
            "P: \"LocalStart\", \"KTime\", \"Time\", \"\",0",
            &format!("P: \"LocalStart\", \"KTime\", \"Time\", \"\",{}", -period),
            1,
        )
        .replacen(
            "P: \"LocalStop\", \"KTime\", \"Time\", \"\",46186158000",
            &format!("P: \"LocalStop\", \"KTime\", \"Time\", \"\",{end_ticks}"),
            1,
        );
        let path = PathBuf::from(format!("exact-{suffix}.fbx"));
        let loaded = animsmith_fbx::load_source_bytes(&path, source.as_bytes())
            .expect("standard-basis analytic fixture loads");
        let timing = loaded.exact_fbx_timing().expect("exact FBX evidence");
        assert_eq!(
            timing.ktime_basis().state(),
            &ExactFbxTimingObservationStateV1::Observed(
                animsmith_core::FbxKTimeBasisV1::new(FBX_KTIME_STANDARD_TICKS_PER_SECOND).unwrap()
            )
        );
        assert_eq!(
            timing.declared_time_mode().state(),
            &ExactFbxTimingObservationStateV1::Observed(FbxTimeModeV1::NtscDropFrame)
        );
        assert_eq!(
            timing.declared_time_protocol().state(),
            &ExactFbxTimingObservationStateV1::Observed(FbxTimeProtocolV1::Smpte)
        );
        let ExactFbxTimingObservationStateV1::Observed(frame_period) =
            timing.frame_period().state()
        else {
            panic!("exact NTSC period");
        };
        assert_eq!(frame_period.ticks_per_frame(), period);
        let ExactFbxTimingObservationStateV1::Observed(range) =
            timing.stacks()[0].source_tick_range().state()
        else {
            panic!("exact signed range");
        };
        assert_eq!(range.begin_ticks(), -period);
        assert_eq!(range.end_ticks(), end_ticks);
        assert_eq!(frame_period.is_whole_frame(range.end_ticks()), whole);
    }
}

#[test]
fn exact_fbx_timing_reproduces_pair_fallback_without_mixing_or_malformed_substitution() {
    let reference_start = -123i64;
    let reference_stop = 456i64;
    let incomplete_local = analytic_timing_fixture(&[])
        .replace(
            "\t\t\tP: \"LocalStop\", \"KTime\", \"Time\", \"\",46186158000\n",
            "",
        )
        .replacen(
            "P: \"ReferenceStart\", \"KTime\", \"Time\", \"\",0",
            &format!("P: \"ReferenceStart\", \"KTime\", \"Time\", \"\",{reference_start}"),
            1,
        )
        .replacen(
            "P: \"ReferenceStop\", \"KTime\", \"Time\", \"\",46186158000",
            &format!("P: \"ReferenceStop\", \"KTime\", \"Time\", \"\",{reference_stop}"),
            1,
        );
    let loaded = animsmith_fbx::load_source_bytes(
        PathBuf::from("reference-fallback.fbx").as_path(),
        incomplete_local.as_bytes(),
    )
    .expect("reference fallback fixture loads");
    let ExactFbxTimingObservationStateV1::Observed(range) =
        loaded.exact_fbx_timing().expect("exact evidence").stacks()[0]
            .source_tick_range()
            .state()
    else {
        panic!("reference range");
    };
    assert_eq!(range.selection(), FbxTimeSpanSelectionV1::Reference);
    assert_eq!((range.begin_ticks(), range.end_ticks()), (-123, 456));

    let malformed_local = analytic_timing_fixture(&[])
        .replacen(
            "P: \"LocalStart\", \"KTime\", \"Time\", \"\",0",
            "P: \"LocalStart\", \"KTime\", \"Time\", \"\",10",
            1,
        )
        .replacen(
            "P: \"LocalStop\", \"KTime\", \"Time\", \"\",46186158000",
            "P: \"LocalStop\", \"KTime\", \"Time\", \"\",9",
            1,
        );
    let loaded = animsmith_fbx::load_source_bytes(
        PathBuf::from("malformed-local.fbx").as_path(),
        malformed_local.as_bytes(),
    )
    .expect("ufbx still loads reversed stack markers");
    assert_eq!(
        loaded.exact_fbx_timing().expect("exact evidence").stacks()[0]
            .source_tick_range()
            .state(),
        &ExactFbxTimingObservationStateV1::Unavailable(
            ExactFbxTimingUnavailableReasonV1::Malformed
        )
    );
}

#[test]
fn exact_fbx_timing_does_not_promote_ufbx_silent_zero_or_wrong_type() {
    let absent = [
        "\t\t\tP: \"LocalStart\", \"KTime\", \"Time\", \"\",0\n",
        "\t\t\tP: \"LocalStop\", \"KTime\", \"Time\", \"\",46186158000\n",
        "\t\t\tP: \"ReferenceStart\", \"KTime\", \"Time\", \"\",0\n",
        "\t\t\tP: \"ReferenceStop\", \"KTime\", \"Time\", \"\",46186158000\n",
    ]
    .into_iter()
    .fold(analytic_timing_fixture(&[]), |source, property| {
        source.replace(property, "")
    });
    let loaded = animsmith_fbx::load_source_bytes(
        PathBuf::from("absent-stack-times.fbx").as_path(),
        absent.as_bytes(),
    )
    .expect("stack without time markers loads");
    assert_eq!(
        loaded.source_facts().clips().rows()[0]
            .source_range()
            .state(),
        &SourceObservationStateV1::Observed(
            animsmith_core::SourceTimeRangeV1::new(0.0, 0.0).unwrap()
        )
    );
    assert_eq!(
        loaded.exact_fbx_timing().expect("exact evidence").stacks()[0]
            .source_tick_range()
            .state(),
        &ExactFbxTimingObservationStateV1::ProvenAbsent
    );

    let wrong_type = analytic_timing_fixture(&[]).replacen(
        "P: \"LocalStart\", \"KTime\", \"Time\", \"\",0",
        "P: \"LocalStart\", \"KString\", \"Time\", \"\",0",
        1,
    );
    let loaded = animsmith_fbx::load_source_bytes(
        PathBuf::from("wrong-type-local.fbx").as_path(),
        wrong_type.as_bytes(),
    )
    .expect("wrong-type time marker remains parser-loadable");
    assert_eq!(
        loaded.exact_fbx_timing().expect("exact evidence").stacks()[0]
            .source_tick_range()
            .state(),
        &ExactFbxTimingObservationStateV1::Unavailable(
            ExactFbxTimingUnavailableReasonV1::Malformed
        )
    );
}

#[test]
fn exact_fbx_timing_distinguishes_explicit_absent_custom_invalid_and_unsupported_modes() {
    struct Case {
        name: &'static str,
        property: Option<&'static str>,
        declared: ExactFbxTimingObservationStateV1<FbxTimeModeV1>,
        effective: FbxTimeModeV1,
        period: Result<i64, ExactFbxTimingUnavailableReasonV1>,
    }
    let cases = [
        Case {
            name: "explicit-default",
            property: Some("\t\tP: \"TimeMode\", \"enum\", \"\", \"\",0"),
            declared: ExactFbxTimingObservationStateV1::Observed(FbxTimeModeV1::Default),
            effective: FbxTimeModeV1::Default,
            period: Ok(FBX_KTIME_LEGACY_TICKS_PER_SECOND / 30),
        },
        Case {
            name: "absent",
            property: None,
            declared: ExactFbxTimingObservationStateV1::ProvenAbsent,
            effective: FbxTimeModeV1::Fps24,
            period: Ok(FBX_KTIME_LEGACY_TICKS_PER_SECOND / 24),
        },
        Case {
            name: "custom",
            property: Some(concat!(
                "\t\tP: \"TimeMode\", \"enum\", \"\", \"\",14\n",
                "\t\tP: \"CustomFrameRate\", \"double\", \"Number\", \"\",23.5"
            )),
            declared: ExactFbxTimingObservationStateV1::Observed(FbxTimeModeV1::Custom),
            effective: FbxTimeModeV1::Custom,
            period: Err(ExactFbxTimingUnavailableReasonV1::CustomFrameRateNotExact),
        },
        Case {
            name: "invalid",
            property: Some("\t\tP: \"TimeMode\", \"enum\", \"\", \"\",99"),
            declared: ExactFbxTimingObservationStateV1::Unavailable(
                ExactFbxTimingUnavailableReasonV1::UnsupportedTimeMode,
            ),
            effective: FbxTimeModeV1::Fps24,
            period: Ok(FBX_KTIME_LEGACY_TICKS_PER_SECOND / 24),
        },
        Case {
            name: "legacy-72",
            property: Some("\t\tP: \"TimeMode\", \"enum\", \"\", \"\",16"),
            declared: ExactFbxTimingObservationStateV1::Observed(FbxTimeModeV1::Fps72),
            effective: FbxTimeModeV1::Fps72,
            period: Err(ExactFbxTimingUnavailableReasonV1::UnsupportedKTimeBasis),
        },
    ];

    for case in cases {
        let properties = case.property.into_iter().collect::<Vec<_>>();
        let source = analytic_timing_fixture(&properties);
        let loaded = animsmith_fbx::load_source_bytes(
            PathBuf::from(format!("{}.fbx", case.name)).as_path(),
            source.as_bytes(),
        )
        .expect("time-mode analytic fixture loads");
        let timing = loaded.exact_fbx_timing().expect("exact evidence");
        assert_eq!(timing.declared_time_mode().state(), &case.declared);
        assert_eq!(
            timing.effective_time_mode().state(),
            &ExactFbxTimingObservationStateV1::Observed(case.effective)
        );
        match case.period {
            Ok(expected) => {
                let ExactFbxTimingObservationStateV1::Observed(period) =
                    timing.frame_period().state()
                else {
                    panic!("{} has exact period", case.name);
                };
                assert_eq!(period.ticks_per_frame(), expected, "{}", case.name);
            }
            Err(expected) => assert_eq!(
                timing.frame_period().state(),
                &ExactFbxTimingObservationStateV1::Unavailable(expected),
                "{}",
                case.name
            ),
        }
        if case.name == "custom" {
            let ExactFbxTimingObservationStateV1::Observed(custom_rate) =
                timing.declared_custom_frame_rate().state()
            else {
                panic!("custom rate binary64 evidence");
            };
            assert_eq!(custom_rate.binary64_bits(), 23.5f64.to_bits());
        } else {
            assert_eq!(
                timing.declared_custom_frame_rate().state(),
                &ExactFbxTimingObservationStateV1::ProvenAbsent,
                "{}",
                case.name
            );
        }
    }

    let malformed_custom = analytic_timing_fixture(&[concat!(
        "\t\tP: \"TimeMode\", \"enum\", \"\", \"\",14\n",
        "\t\tP: \"CustomFrameRate\", \"double\", \"Number\", \"\",-1"
    )]);
    let loaded = animsmith_fbx::load_source_bytes(
        PathBuf::from("malformed-custom.fbx").as_path(),
        malformed_custom.as_bytes(),
    )
    .expect("malformed custom-rate evidence does not prevent parsing");
    let timing = loaded.exact_fbx_timing().expect("exact evidence");
    assert_eq!(
        timing.declared_custom_frame_rate().state(),
        &ExactFbxTimingObservationStateV1::Unavailable(
            ExactFbxTimingUnavailableReasonV1::Malformed
        )
    );
    assert_eq!(
        timing.frame_period().state(),
        &ExactFbxTimingObservationStateV1::Unavailable(
            ExactFbxTimingUnavailableReasonV1::CustomFrameRateNotExact
        )
    );
}

#[test]
fn empty_ufbx_take_name_does_not_overclaim_source_absence() {
    let source = std::fs::read_to_string(fixture())
        .expect("read self-authored fixture")
        .replacen("\"AnimStack::take\"", "\"AnimStack::\"", 1);
    let loaded =
        animsmith_fbx::load_source_bytes(PathBuf::from("unnamed.fbx").as_path(), source.as_bytes())
            .expect("unnamed take fixture loads");
    let [take] = loaded.source_facts().clips().rows() else {
        panic!("one source take");
    };
    assert_eq!(
        take.source_name().state(),
        &SourceObservationStateV1::Unavailable(SourceUnavailableReasonV1::ParserUnavailable)
    );
}

#[test]
fn user_defined_properties_are_reported_as_an_unsupported_aggregate() {
    let source = std::fs::read_to_string(fixture())
        .expect("read self-authored fixture")
        .replacen(
            "P: \"Lcl Translation\", \"Lcl Translation\", \"\", \"A\",0,0,0",
            concat!(
                "P: \"ExampleProperty\", \"KString\", \"\", \"U\",\"example\"\n",
                "\t\t\tP: \"Lcl Translation\", \"Lcl Translation\", \"\", \"A\",0,0,0"
            ),
            1,
        );
    let loaded = animsmith_fbx::load_scale_source_bytes(
        PathBuf::from("custom-property.fbx").as_path(),
        source.as_bytes(),
    )
    .expect("custom-property fixture loads");
    assert_eq!(loaded.inventory().user_defined_property_count, 1);
    let [custom] = loaded.source_facts().constructs().rows() else {
        panic!("one custom-property aggregate");
    };
    assert_eq!(custom.source_order_index(), 0);
    assert_eq!(custom.kind(), SourceConstructKindV1::CustomProperty);
    assert_eq!(custom.name().as_str(), "fbx:user-defined-properties");
    assert_eq!(custom.count(), 1);
    assert_eq!(custom.disposition(), SourceLoaderDispositionV1::Unsupported);
    let facts = animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect("discarded custom properties do not obscure the normalized rest/bind domain");
    assert!(!facts.extras_present);
}

#[test]
fn effective_fbx_units_and_basis_do_not_promote_advisory_original_fields() {
    let source = std::fs::read_to_string(fixture())
        .expect("read self-authored fixture")
        .replacen(
            "P: \"OriginalUnitScaleFactor\", \"double\", \"Number\", \"\",1",
            concat!(
                "P: \"OriginalUnitScaleFactor\", \"double\", \"Number\", \"\",100\n",
                "\t\tP: \"OriginalUpAxis\", \"int\", \"Integer\", \"\",2\n",
                "\t\tP: \"OriginalUpAxisSign\", \"int\", \"Integer\", \"\",1"
            ),
            1,
        );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("advisory-original-fields.fbx");
    std::fs::write(&path, &source).expect("write analytic advisory fixture");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("advisory fixture loads");
    assert_eq!(
        loaded
            .inventory()
            .coordinate_normalization
            .original_unit_meters,
        1.0
    );
    assert_eq!(
        loaded.inventory().coordinate_normalization.original_up_axis,
        FbxCoordinateAxis::PositiveZ
    );

    let facts = loaded.source_facts();
    let SourceObservationStateV1::Observed(unit) = facts.linear_unit().state() else {
        panic!("effective unit is available");
    };
    assert_eq!(unit.meters_per_source_unit(), 0.01);
    let SourceObservationStateV1::Observed(basis) = facts.coordinate_basis().state() else {
        panic!("effective basis is available");
    };
    assert_eq!(basis.right(), SourceAxisV1::PositiveX);
    assert_eq!(basis.up(), SourceAxisV1::PositiveY);
    assert_eq!(basis.forward(), SourceAxisV1::PositiveZ);
    assert_eq!(
        facts.linear_unit().disposition(),
        SourceLoaderDispositionV1::Normalized
    );
    assert_eq!(
        facts.coordinate_basis().disposition(),
        SourceLoaderDispositionV1::Normalized
    );
    let SourceObservationStateV1::Observed(fps) = facts.frames_per_second().state() else {
        panic!("finite parser FPS is available");
    };
    assert!(fps.get().is_finite() && fps.get() > 0.0);
}

#[test]
fn source_take_range_and_translation_property_stay_distinct_from_baked_tracks() {
    const SECOND: &str = "46186158000";
    const TWO_SECONDS: &str = "92372316000";
    let source = std::fs::read_to_string(fixture())
        .expect("read self-authored fixture")
        .replacen(
            "P: \"LocalStart\", \"KTime\", \"Time\", \"\",0",
            &format!("P: \"LocalStart\", \"KTime\", \"Time\", \"\",{SECOND}"),
            1,
        )
        .replacen(
            &format!("P: \"LocalStop\", \"KTime\", \"Time\", \"\",{SECOND}"),
            &format!("P: \"LocalStop\", \"KTime\", \"Time\", \"\",{TWO_SECONDS}"),
            1,
        )
        .replacen(
            "P: \"ReferenceStart\", \"KTime\", \"Time\", \"\",0",
            &format!("P: \"ReferenceStart\", \"KTime\", \"Time\", \"\",{SECOND}"),
            1,
        )
        .replacen(
            &format!("P: \"ReferenceStop\", \"KTime\", \"Time\", \"\",{SECOND}"),
            &format!("P: \"ReferenceStop\", \"KTime\", \"Time\", \"\",{TWO_SECONDS}"),
            1,
        )
        .replacen(
            &format!("KeyTime: *2 {{ a: 0,{SECOND} }}"),
            &format!("KeyTime: *2 {{ a: {SECOND},{TWO_SECONDS} }}"),
            1,
        );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("one-to-two-second-take.fbx");
    std::fs::write(&path, source).expect("write analytic nonzero-range fixture");

    let loaded = animsmith_fbx::load_source(&path).expect("nonzero-range fixture loads");
    let facts = loaded.source_facts();
    let [take] = facts.clips().rows() else {
        panic!("one source take");
    };
    let SourceObservationStateV1::Observed(range) = take.source_range().state() else {
        panic!("ufbx parser-resolved take range");
    };
    assert!((range.begin_s() - 1.0).abs() < 1e-9);
    assert!((range.end_s() - 2.0).abs() < 1e-9);
    let SourceObservationStateV1::Observed(name) = take.source_name().state() else {
        panic!("raw optional take name");
    };
    assert_eq!(name.as_str(), "take");
    assert_eq!(
        take.normalized_clip_index().state(),
        &SourceObservationStateV1::Observed(0)
    );

    let [channel] = take.channels().rows() else {
        panic!("the fixture authors one layer/property binding");
    };
    assert_eq!(channel.source_channel_index(), 0);
    assert_eq!(channel.source_layer_index(), Some(0));
    assert_eq!(channel.property(), SourceChannelPropertyV1::Translation);
    assert!(channel.components().x());
    assert!(!channel.components().y());
    assert!(!channel.components().z());
    assert_eq!(channel.disposition(), SourceLoaderDispositionV1::Baked);
    assert_eq!(
        channel.interpolation().state(),
        &SourceObservationStateV1::Unavailable(SourceUnavailableReasonV1::BakedAway)
    );

    let clip = &loaded.document().clips[0];
    assert!((clip.duration_s - 1.0).abs() < 1e-6);
    assert!(
        clip.tracks
            .iter()
            .any(|track| { track.property == Property::Rotation && !track.times.is_empty() })
    );
    assert!(
        clip.tracks
            .iter()
            .any(|track| { track.property == Property::Scale && !track.times.is_empty() })
    );
    let translation = clip
        .tracks
        .iter()
        .find(|track| track.bone == 1 && track.property == Property::Translation)
        .expect("baked translation track");
    assert_eq!(translation.times.first().copied(), Some(0.0));
    assert_eq!(translation.times.last().copied(), Some(1.0));
}

#[test]
fn captured_bytes_survive_primary_path_removal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("captured.fbx");
    let bytes = std::fs::read(fixture()).expect("capture fixture");
    std::fs::write(&path, &bytes).expect("write primary input");
    std::fs::remove_file(&path).expect("remove primary input after capture");

    let doc = animsmith_fbx::load_bytes(&path, &bytes).expect("captured bytes still load");

    assert_eq!(
        doc.source.path.as_deref(),
        Some(path.to_str().expect("UTF-8 path"))
    );
    assert_eq!(doc.clips[0].name, "take");
}

fn write_normal_material(dir: &tempfile::TempDir, image: NormalImage) -> PathBuf {
    let source_dir = dir.path().join("scene");
    std::fs::create_dir(&source_dir).expect("create source directory");
    let source = std::fs::read_to_string(fixture())
        .expect("read fixture")
        .replace("\r\n", "\n");
    let source = source.replacen(
        "\tObjectType: \"Deformer\" { Count: 2 }\n}",
        "\tObjectType: \"Deformer\" { Count: 2 }\n\tObjectType: \"Material\" { Count: 1 }\n\tObjectType: \"Texture\" { Count: 1 }\n\tObjectType: \"Video\" { Count: 1 }\n}",
        1,
    );
    let content = if matches!(image, NormalImage::Embedded) {
        format!("\n\t\tContent: ,\"{TINY_PNG_B64}\"")
    } else {
        String::new()
    };
    let material_objects = format!(
        r#"	Material: 5001, "Material::normal_mat", "" {{
		Version: 102
		ShadingModel: "phong"
		MultiLayer: 0
	}}
	Texture: 5002, "Texture::normal", "" {{
		Type: "TextureVideoClip"
		Version: 202
		TextureName: "Texture::normal"
		Media: "Video::normal"
		FileName: "normal.png"
		RelativeFilename: "normal.png"
		ModelUVTranslation: 0,0
		ModelUVScaling: 1,1
		Texture_Alpha_Source: "None"
		Cropping: 0,0,0,0
	}}
	Video: 5003, "Video::normal", "Clip" {{
		Type: "Clip"
		Properties70: {{
			P: "Path", "KString", "XRefUrl", "", "normal.png"
		}}
		FileName: "normal.png"
		RelativeFilename: "normal.png"{content}
	}}
}}
Connections: {{"#
    );
    let source = source.replacen("}\nConnections: {", &material_objects, 1);
    let source = source.replacen(
        "Connections: {",
        "Connections: {\n\tC: \"OO\",5001,1002\n\tC: \"OP\",5002,5001,\"NormalMap\"\n\tC: \"OO\",5003,5002",
        1,
    );
    assert!(source.contains("Material::normal_mat"));
    assert!(source.contains("5002,5001,\"NormalMap\""));

    let path = source_dir.join("normal-material.fbx");
    std::fs::write(&path, source).expect("write FBX");
    match image {
        NormalImage::Linked(bytes) => {
            std::fs::write(source_dir.join("normal.png"), bytes).expect("write normal image");
        }
        NormalImage::Embedded => {}
        NormalImage::MissingWithParentDecoy => {
            std::fs::write(dir.path().join("normal.png"), TINY_PNG)
                .expect("write parent-directory decoy");
        }
        NormalImage::Unreadable => {
            std::fs::create_dir(source_dir.join("normal.png"))
                .expect("create unreadable image path");
        }
    }

    path
}

fn load_normal_material(image: NormalImage) -> animsmith_core::Document {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, image);
    animsmith_fbx::load(&path).expect("normal-map FBX loads")
}

fn assert_normal_texture(doc: &animsmith_core::Document) {
    assert_eq!(doc.assets.materials.len(), 1);
    let normal = doc.assets.materials[0]
        .normal_texture
        .as_ref()
        .expect("normal map carried");
    assert_eq!(normal.texture.mime, "image/png");
    assert_eq!(normal.texture.bytes, TINY_PNG);
    assert_eq!(normal.scale, 1.0, "FBX uses glTF default normal scale");
    assert!(doc.assets.materials[0].base_color_texture.is_none());
}

#[test]
fn loads_linked_fbx_normal_texture() {
    assert_normal_texture(&load_normal_material(NormalImage::Linked(TINY_PNG)));
}

#[test]
fn byte_loader_resolves_external_texture_relative_to_supplied_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Linked(TINY_PNG));
    let bytes = std::fs::read(&path).expect("capture FBX");
    std::fs::remove_file(&path).expect("remove primary input after capture");

    let source = animsmith_fbx::load_scale_source_bytes_with_resource_root(
        &path,
        &bytes,
        path.parent().expect("source parent is the explicit root"),
    )
    .expect("captured FBX loads under its explicit resource root");

    assert_eq!(source.inventory().external_resource_count, 2);
    assert!(animsmith_fbx::capability_facts(source.inventory()).external_resources_present);
    assert_normal_texture(source.document());
    let facts = animsmith_fbx::rest_bind_capability_facts_for_source(&source)
        .expect("captured external texture bytes are orthogonal to rest/bind transforms");
    assert!(!facts.external_resources_present);
}

#[test]
fn source_facts_keep_texture_and_video_alias_declarations_separate() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Linked(TINY_PNG));

    let loaded = animsmith_fbx::load_source(&path).expect("linked-resource FBX loads");
    let resources = loaded.source_facts().resources();
    assert_eq!(
        resources.coverage().state(),
        SourceSetCoverageStateV1::Complete
    );
    let [texture, video] = resources.rows() else {
        panic!("texture and video declarations remain separate reference rows");
    };
    assert_eq!(texture.source_order_index(), 0);
    assert_eq!(video.source_order_index(), 1);
    assert_eq!(texture.kind(), SourceResourceKindV1::Texture);
    assert_eq!(video.kind(), SourceResourceKindV1::Video);
    for resource in [texture, video] {
        let SourceResourceLocatorV1::Relative(locator) = resource.locator() else {
            panic!("safe relative declaration spelling is retained");
        };
        assert_eq!(locator.as_str(), "normal.png");
    }
    assert_eq!(texture.disposition(), SourceLoaderDispositionV1::Unknown);
    assert_eq!(video.disposition(), SourceLoaderDispositionV1::Discarded);
}

#[test]
fn rooted_capture_completely_models_and_hashes_a_texture_video_alias_once() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Linked(TINY_PNG));

    let loaded = animsmith_fbx::load_source(&path).expect("linked-resource FBX loads");
    let closure = loaded.dependency_closure();
    assert!(closure.coverage().is_complete());
    assert!(closure.identity().is_some());
    assert_eq!(closure.references().len(), 2);
    assert_eq!(closure.external_resources().len(), 1);
    assert_eq!(closure.external_resources()[0].key().as_str(), "normal.png");
    assert_eq!(closure.work().external_open_attempts(), 1);
    assert_eq!(closure.work().captured_external_resources(), 1);
    assert_eq!(
        closure.work().external_bytes_read_hashed(),
        TINY_PNG.len() as u64
    );
    assert!(matches!(
        closure.references()[0].target(),
        DependencyReferenceTargetV1::External { key } if key.as_str() == "normal.png"
    ));
    assert!(matches!(
        closure.references()[1].target(),
        DependencyReferenceTargetV1::External { key } if key.as_str() == "normal.png"
    ));
    assert_normal_texture(loaded.document());
}

#[test]
fn unmodeled_audio_clip_prevents_a_complete_dependency_closure() {
    let source = std::fs::read_to_string(fixture())
        .expect("read analytic FBX")
        .replace("\r\n", "\n")
        .replacen(
            "\tObjectType: \"Deformer\" { Count: 2 }\n}",
            "\tObjectType: \"Deformer\" { Count: 2 }\n\tObjectType: \"Audio\" { Count: 1 }\n}",
            1,
        )
        .replacen(
            "}\nConnections: {",
            concat!(
                "\tAudio: 9100, \"Audio::voice\", \"Clip\" {\n",
                "\t\tProperties70: {\n",
                "\t\t\tP: \"Path\", \"KString\", \"XRefUrl\", \"\", \"voice.wav\"\n",
                "\t\t\tP: \"RelPath\", \"KString\", \"XRefUrl\", \"\", \"voice.wav\"\n",
                "\t\t}\n",
                "\t}\n",
                "}\nConnections: {"
            ),
            1,
        );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("audio-clip.fbx");
    std::fs::write(&path, source).expect("write audio fixture");
    std::fs::write(dir.path().join("voice.wav"), b"sentinel audio")
        .expect("write sidecar sentinel");

    let loaded = animsmith_fbx::load_scale_source(&path).expect("audio fixture parses");
    let closure = loaded.dependency_closure();
    assert!(!closure.coverage().is_complete());
    assert!(closure.identity().is_none());
    assert!(closure.references().is_empty());
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(
        closure
            .coverage()
            .reasons()
            .contains(&DependencyClosureCoverageReasonV1::UnmodeledResourceDomain)
    );
    let error = animsmith_fbx::rest_bind_capability_facts_for_source(&loaded)
        .expect_err("unmodeled audio must remain outside the rest/bind admission boundary");
    assert_eq!(
        error,
        concat!(
            "FBX rest/bind raw-source facts rejected: ",
            "raw_source.construct=unknown_element(fbx:unmodeled-elements; count=1; audio_clips=1)"
        )
    );
}

#[test]
fn byte_only_capture_records_safe_aliases_as_root_unavailable_without_io() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Linked(TINY_PNG));
    let bytes = std::fs::read(&path).expect("capture primary FBX bytes");

    let loaded = animsmith_fbx::load_source_bytes(&path, &bytes)
        .expect("byte-only FBX load does not require an external root");
    let closure = loaded.dependency_closure();
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(closure.external_resources().is_empty());
    assert_eq!(closure.references().len(), 2);
    assert!(closure.references().iter().all(|reference| {
        matches!(
            reference.target(),
            DependencyReferenceTargetV1::Unavailable {
                key: Some(key),
                reason: DependencyResourceUnavailableReasonV1::ResourceRootUnavailable,
            } if key.as_str() == "normal.png"
        )
    }));
    assert!(
        loaded.document().assets.materials[0]
            .normal_texture
            .is_none()
    );
}

#[test]
fn rooted_capture_keeps_distinct_identical_external_keys_distinct() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Linked(TINY_PNG));
    let source = std::fs::read_to_string(&path)
        .expect("read analytic FBX")
        .replace(
            "\tVideo: 5003, \"Video::normal\", \"Clip\" {\n\t\tType: \"Clip\"\n\t\tProperties70: {\n\t\t\tP: \"Path\", \"KString\", \"XRefUrl\", \"\", \"normal.png\"\n\t\t}\n\t\tFileName: \"normal.png\"\n\t\tRelativeFilename: \"normal.png\"",
            "\tVideo: 5003, \"Video::normal\", \"Clip\" {\n\t\tType: \"Clip\"\n\t\tProperties70: {\n\t\t\tP: \"Path\", \"KString\", \"XRefUrl\", \"\", \"alias.png\"\n\t\t}\n\t\tFileName: \"alias.png\"\n\t\tRelativeFilename: \"alias.png\"",
        );
    assert!(source.contains("RelativeFilename: \"alias.png\""));
    std::fs::write(&path, source).expect("write distinct-alias FBX");
    std::fs::write(path.with_file_name("alias.png"), TINY_PNG).expect("write identical alias");

    let loaded = animsmith_fbx::load_source(&path).expect("distinct-alias FBX loads");
    let closure = loaded.dependency_closure();
    assert_eq!(closure.references().len(), 2);
    assert_eq!(closure.external_resources().len(), 2);
    assert_eq!(closure.work().external_open_attempts(), 2);
    assert_eq!(closure.work().captured_external_resources(), 2);
    assert_eq!(closure.external_resources()[0].key().as_str(), "alias.png");
    assert_eq!(closure.external_resources()[1].key().as_str(), "normal.png");
    assert_eq!(
        closure.external_resources()[0].identity(),
        closure.external_resources()[1].identity(),
        "equal bytes do not collapse distinct logical keys"
    );
}

#[test]
fn rooted_capture_hashes_safe_relative_cache_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("cache-file.fbx");
    let cache_bytes = b"analytic cache payload";
    let source = std::fs::read_to_string(fixture())
        .expect("read analytic FBX")
        .replace("\r\n", "\n")
        .replacen(
            "\tObjectType: \"Deformer\" { Count: 2 }\n}",
            "\tObjectType: \"Deformer\" { Count: 2 }\n\tObjectType: \"Cache\" { Count: 1 }\n}",
            1,
        )
        .replacen(
            "}\nConnections: {",
            concat!(
                "\tCache: 9000, \"Cache::analytic\", \"\" {\n",
                "\t\tProperties70: {\n",
                "\t\t\tP: \"CacheFileName\", \"KString\", \"\", \"\", \"cache.pc2\"\n",
                "\t\t\tP: \"CacheFileType\", \"int\", \"Integer\", \"\",1\n",
                "\t\t}\n",
                "\t}\n",
                "}\nConnections: {"
            ),
            1,
        );
    std::fs::write(&path, source).expect("write cache-file FBX");
    std::fs::write(path.with_file_name("cache.pc2"), cache_bytes).expect("write cache bytes");

    let loaded = animsmith_fbx::load_source(&path).expect("cache-file FBX loads");
    let resources = loaded.source_facts().resources();
    let [cache] = resources.rows() else {
        panic!("one parser-projected cache declaration: {resources:?}");
    };
    assert_eq!(cache.kind(), SourceResourceKindV1::Cache);
    assert!(matches!(
        cache.locator(),
        SourceResourceLocatorV1::Relative(locator) if locator.as_str() == "cache.pc2"
    ));
    let closure = loaded.dependency_closure();
    assert!(closure.coverage().is_complete());
    assert_eq!(closure.references().len(), 1);
    assert_eq!(closure.external_resources().len(), 1);
    assert_eq!(closure.work().external_open_attempts(), 1);
    assert_eq!(closure.external_resources()[0].key().as_str(), "cache.pc2");
    assert_eq!(
        closure.external_resources()[0].identity(),
        &InputIdentity::from_bytes(cache_bytes)
    );
}

#[derive(Clone, Copy, Debug)]
enum RedactedLocatorCase {
    Escaping,
    Remote,
    Malformed,
    Oversized,
}

#[test]
fn unsafe_resource_locator_table_is_refused_without_io_or_spelling_leaks() {
    let oversized = "x".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES + 1);
    let cases = [
        (
            "traversal",
            "../secret-token",
            RedactedLocatorCase::Escaping,
        ),
        (
            "remote",
            "https://example.invalid/secret-token",
            RedactedLocatorCase::Remote,
        ),
        (
            "encoded-separator",
            "folder%2Fsecret-token",
            RedactedLocatorCase::Escaping,
        ),
        (
            "control",
            "folder\u{0007}secret-token",
            RedactedLocatorCase::Malformed,
        ),
        (
            "malformed-percent",
            "folder%zzsecret-token",
            RedactedLocatorCase::Malformed,
        ),
        (
            "oversized",
            oversized.as_str(),
            RedactedLocatorCase::Oversized,
        ),
    ];

    for (label, locator, expected) in cases {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write_normal_material(&dir, NormalImage::Linked(TINY_PNG));
        let source = std::fs::read_to_string(&path)
            .expect("read analytic FBX")
            .replace(
                "RelativeFilename: \"normal.png\"",
                &format!("RelativeFilename: \"{locator}\""),
            );
        std::fs::write(&path, source).expect("write unsafe-locator FBX");

        let loaded = animsmith_fbx::load_source(&path)
            .unwrap_or_else(|error| panic!("{label}: parser accepts analytic locator: {error}"));
        let resources = loaded.source_facts().resources();
        assert_eq!(
            resources.rows().len(),
            2,
            "{label}: both declarations remain"
        );
        for resource in resources.rows() {
            assert!(
                matches_locator_case(resource.locator(), expected),
                "{label}: unexpected locator classification: {:?}",
                resource.locator()
            );
        }
        let closure = loaded.dependency_closure();
        assert_eq!(closure.work().external_open_attempts(), 0, "{label}");
        assert!(closure.external_resources().is_empty(), "{label}");
        assert!(
            closure
                .references()
                .iter()
                .all(|reference| matches_refusal_case(reference.target(), expected))
        );
        let debug = format!("{:?}{:?}", loaded.source_facts(), closure);
        assert!(!debug.contains("secret-token"), "{label}: {debug}");
        assert!(
            loaded.document().assets.materials[0]
                .normal_texture
                .is_none()
        );
    }
}

fn matches_locator_case(locator: &SourceResourceLocatorV1, expected: RedactedLocatorCase) -> bool {
    matches!(
        (locator, expected),
        (
            SourceResourceLocatorV1::Escaping,
            RedactedLocatorCase::Escaping
        ) | (SourceResourceLocatorV1::Remote, RedactedLocatorCase::Remote)
            | (
                SourceResourceLocatorV1::Malformed,
                RedactedLocatorCase::Malformed
            )
            | (
                SourceResourceLocatorV1::Oversized,
                RedactedLocatorCase::Oversized
            )
    )
}

fn matches_refusal_case(
    target: &DependencyReferenceTargetV1,
    expected: RedactedLocatorCase,
) -> bool {
    matches!(
        (target, expected),
        (
            DependencyReferenceTargetV1::Refused {
                key: None,
                reason: DependencyResourceRefusalReasonV1::Escaping,
            },
            RedactedLocatorCase::Escaping
        ) | (
            DependencyReferenceTargetV1::Refused {
                key: None,
                reason: DependencyResourceRefusalReasonV1::Remote,
            },
            RedactedLocatorCase::Remote
        ) | (
            DependencyReferenceTargetV1::Refused {
                key: None,
                reason: DependencyResourceRefusalReasonV1::Malformed,
            },
            RedactedLocatorCase::Malformed
        ) | (
            DependencyReferenceTargetV1::Refused {
                key: None,
                reason: DependencyResourceRefusalReasonV1::Oversized,
            },
            RedactedLocatorCase::Oversized
        )
    )
}

#[cfg(unix)]
#[test]
fn rooted_capture_refuses_symlink_without_an_open_attempt() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Linked(TINY_PNG));
    let image = path.with_file_name("normal.png");
    let outside = tempfile::tempdir().expect("outside temp dir");
    let outside_image = outside.path().join("outside.png");
    std::fs::write(&outside_image, TINY_PNG).expect("write outside image");
    std::fs::remove_file(&image).expect("replace linked image with symlink");
    std::os::unix::fs::symlink(&outside_image, &image).expect("create resource symlink");

    let loaded = animsmith_fbx::load_source(&path).expect("symlinked FBX still parses");
    let closure = loaded.dependency_closure();
    assert!(!closure.coverage().is_complete());
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert_eq!(closure.work().captured_external_resources(), 0);
    assert!(
        closure
            .coverage()
            .reasons()
            .contains(&DependencyClosureCoverageReasonV1::RefusedResource)
    );
    assert!(closure.references().iter().all(|reference| {
        matches!(
            reference.target(),
            DependencyReferenceTargetV1::Refused {
                key: Some(key),
                reason: animsmith_core::DependencyResourceRefusalReasonV1::Symlink,
            } if key.as_str() == "normal.png"
        )
    }));
    assert!(
        loaded.document().assets.materials[0]
            .normal_texture
            .is_none()
    );
}

#[test]
fn rooted_capture_does_not_open_a_non_regular_resource_target() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Unreadable);

    let loaded = animsmith_fbx::load_source(&path).expect("directory-resource FBX still parses");
    let closure = loaded.dependency_closure();
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(closure.external_resources().is_empty());
    assert!(closure.references().iter().all(|reference| {
        matches!(
            reference.target(),
            DependencyReferenceTargetV1::Unavailable {
                reason: DependencyResourceUnavailableReasonV1::Unreadable,
                ..
            }
        )
    }));
    assert!(
        loaded.document().assets.materials[0]
            .normal_texture
            .is_none()
    );
}

#[test]
fn absolute_resource_sentinel_is_refused_without_a_host_probe() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Linked(TINY_PNG));
    let source = std::fs::read_to_string(&path)
        .expect("read analytic FBX")
        .replace(
            "RelativeFilename: \"normal.png\"",
            "RelativeFilename: \"/sentinel.png\"",
        );
    std::fs::write(&path, source).expect("write absolute-sentinel FBX");

    let loaded = animsmith_fbx::load_source(&path).expect("absolute-sentinel FBX parses");
    let closure = loaded.dependency_closure();
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(
        closure
            .coverage()
            .reasons()
            .contains(&DependencyClosureCoverageReasonV1::RefusedResource)
    );
    assert!(closure.references().iter().all(|reference| {
        matches!(
            reference.target(),
            DependencyReferenceTargetV1::Refused {
                key: None,
                reason: animsmith_core::DependencyResourceRefusalReasonV1::Absolute,
            }
        )
    }));
    assert!(
        loaded.document().assets.materials[0]
            .normal_texture
            .is_none()
    );
    assert!(!format!("{:?}", closure).contains("sentinel.png"));
}

#[test]
fn file_name_only_fbx_resources_are_redacted_and_never_captureable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Linked(TINY_PNG));
    let source = std::fs::read_to_string(&path)
        .expect("read analytic FBX")
        .replace("\t\tRelativeFilename: \"normal.png\"\n", "");
    std::fs::write(&path, source).expect("write FileName-only FBX");

    let loaded = animsmith_fbx::load_source(&path).expect("FileName-only FBX loads");
    let resources = loaded.source_facts().resources();
    assert_eq!(
        resources.coverage().state(),
        SourceSetCoverageStateV1::Complete
    );
    let [texture, video] = resources.rows() else {
        panic!("texture and video declarations remain present");
    };
    assert!(matches!(
        texture.locator(),
        SourceResourceLocatorV1::Absolute
    ));
    assert!(matches!(video.locator(), SourceResourceLocatorV1::Absolute));
    let debug = format!("{:?}", loaded.source_facts());
    assert!(!debug.contains("normal.png"), "{debug}");
    let closure = loaded.dependency_closure();
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(closure.references().iter().all(|reference| {
        matches!(
            reference.target(),
            DependencyReferenceTargetV1::Refused {
                key: None,
                reason: animsmith_core::DependencyResourceRefusalReasonV1::Absolute,
            }
        )
    }));
    // `normal.png` remains beside the source as a sentinel for the parser's
    // resolved absolute path. It must never be opened through that field.
    assert!(
        loaded.document().assets.materials[0]
            .normal_texture
            .is_none()
    );
}

#[test]
fn resource_projection_n_plus_one_is_partial_without_breaking_legacy_load() {
    let mut videos = String::new();
    for index in 0..=RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES {
        videos.push_str(&format!(
            concat!(
                "\tVideo: {}, \"Video::resource{}\", \"Clip\" {{\n",
                "\t\tType: \"Clip\"\n",
                "\t\tFileName: \"resource{}.bin\"\n",
                "\t\tRelativeFilename: \"resource{}.bin\"\n",
                "\t}}\n"
            ),
            10_000 + index,
            index,
            index,
            index
        ));
    }
    let source = std::fs::read_to_string(fixture())
        .expect("read self-authored fixture")
        .replace("\r\n", "\n")
        .replacen(
            "\tObjectType: \"Deformer\" { Count: 2 }\n}",
            &format!(
                "\tObjectType: \"Deformer\" {{ Count: 2 }}\n\tObjectType: \"Video\" {{ Count: {} }}\n}}",
                RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES + 1
            ),
            1,
        )
        .replacen("}\nConnections: {", &format!("{videos}}}\nConnections: {{"), 1);
    let path = PathBuf::from("resource-budget.fbx");
    let bytes = source.as_bytes();

    let legacy = animsmith_fbx::load_bytes(&path, bytes)
        .expect("projection overflow must not turn legacy success into failure");
    assert_eq!(legacy.clips.len(), 1);
    let loaded = animsmith_fbx::load_source_bytes(&path, bytes)
        .expect("bounded source projection also succeeds");
    let resources = loaded.source_facts().resources();
    assert_eq!(
        resources.rows().len(),
        RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES
    );
    assert_eq!(
        resources.coverage().state(),
        SourceSetCoverageStateV1::Partial
    );
    assert_eq!(
        resources.coverage().reason(),
        Some(SourceUnavailableReasonV1::ProjectionBudgetExceeded)
    );
    assert_eq!(resources.rows()[0].source_index(), 0);
    assert_eq!(resources.rows()[0].source_order_index(), 0);
    assert_eq!(
        resources.rows()[RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES - 1].source_index(),
        (RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES - 1) as u64
    );
    assert_eq!(
        resources.rows()[RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES - 1].source_order_index(),
        RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES - 1
    );
    assert_eq!(
        loaded.source_facts().work().inspected_rows(),
        // One take row, one authored property row, N retained resources, and
        // the terminal resource inspection that establishes partial coverage.
        RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES + 3
    );
    assert_eq!(
        animsmith_fbx::rest_bind_capability_facts_for_source(
            &animsmith_fbx::load_scale_source_bytes(&path, bytes)
                .expect("scale source retains the bounded partial projection")
        )
        .unwrap_err(),
        "FBX rest/bind raw-source facts rejected: raw_source.resources.coverage=partial"
    );
    let closure = loaded.dependency_closure();
    assert_eq!(
        closure.references().len(),
        DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES
    );
    assert_eq!(
        closure.work().inspected_references(),
        DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES + 1
    );
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(
        closure
            .coverage()
            .reasons()
            .contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsPartial)
    );
    assert!(
        closure
            .coverage()
            .reasons()
            .contains(&DependencyClosureCoverageReasonV1::ResourceBudgetExceeded)
    );
}

#[test]
fn loads_embedded_fbx_normal_texture() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Embedded);
    let source = animsmith_fbx::load_scale_source(&path).expect("embedded FBX loads");
    assert_eq!(source.inventory().external_resource_count, 0);
    assert_normal_texture(source.document());
}

#[test]
fn loads_linked_fbx_jpeg_normal_texture_without_rewriting_bytes() {
    let doc = load_normal_material(NormalImage::Linked(TINY_JPEG));
    let normal = doc.assets.materials[0]
        .normal_texture
        .as_ref()
        .expect("JPEG normal map carried");
    assert_eq!(normal.texture.mime, "image/jpeg");
    assert_eq!(normal.texture.bytes, TINY_JPEG);
}

#[test]
fn missing_unreadable_and_unsupported_fbx_normal_images_are_not_guessed_or_injected() {
    for (label, image) in [
        (
            "missing-with-parent-decoy",
            NormalImage::MissingWithParentDecoy,
        ),
        ("unreadable", NormalImage::Unreadable),
        ("unsupported", NormalImage::Linked(b"not an image")),
    ] {
        let doc = load_normal_material(image);
        assert!(
            doc.assets.materials[0].normal_texture.is_none(),
            "{label}: no normal texture should be invented"
        );
    }
}

#[test]
fn garbage_file_is_reported_as_fbx_parse_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("garbage.fbx");
    std::fs::write(&path, b"not an fbx file").expect("write garbage input");

    let err = animsmith_fbx::load(&path).expect_err("garbage input should not load");
    assert!(
        matches!(err, animsmith_fbx::LoadError::Fbx(_)),
        "expected LoadError::Fbx, got {err:?}"
    );
}

#[test]
fn normalizes_centimetre_z_up_scene_to_metre_y_up() {
    let source = std::fs::read_to_string(fixture()).expect("read self-authored fixture");
    let source = source.replacen(
        "P: \"UpAxis\", \"int\", \"Integer\", \"\",1",
        "P: \"UpAxis\", \"int\", \"Integer\", \"\",2",
        1,
    );
    let source = source.replacen(
        "P: \"FrontAxis\", \"int\", \"Integer\", \"\",2",
        "P: \"FrontAxis\", \"int\", \"Integer\", \"\",1",
        1,
    );
    let source = source.replacen(
        "P: \"FrontAxisSign\", \"int\", \"Integer\", \"\",1",
        "P: \"FrontAxisSign\", \"int\", \"Integer\", \"\",-1",
        1,
    );
    let source = source.replacen(
        "Vertices: *9 { a: 0,0,0,100,0,0,0,100,0 }",
        "Vertices: *9 { a: 0,0,0,100,0,0,0,100,100 }",
        1,
    );
    let source = source.replacen(
        "C: \"OP\",3004,3003,\"d|X\"",
        "C: \"OP\",3004,3003,\"d|Z\"",
        1,
    );
    assert!(source.contains("\"UpAxis\", \"int\", \"Integer\", \"\",2"));
    assert!(source.contains("\"FrontAxis\", \"int\", \"Integer\", \"\",1"));
    assert!(source.contains("\"FrontAxisSign\", \"int\", \"Integer\", \"\",-1"));
    assert!(source.contains("0,100,100"));
    assert!(source.contains("3004,3003,\"d|Z\""));

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("centimetre-z-up.fbx");
    std::fs::write(&path, source).expect("write transformed fixture");
    let doc = animsmith_fbx::load(&path).expect("Z-up fixture loads");

    let mesh = doc.assets.meshes.first().expect("mesh loaded");
    let primitive = mesh.primitives.first().expect("primitive loaded");
    let instance = doc.assets.instances.first().expect("mesh instance loaded");
    let model = rest_models(&doc)[instance.node];
    let source_x = model.transform_point3(primitive.positions[1]);
    let source_yz = model.transform_point3(primitive.positions[2]);

    assert_vec3_near(source_x, Vec3::X);
    assert_vec3_near(source_yz, Vec3::Y - Vec3::Z);

    let translation = doc.clips[0]
        .tracks
        .iter()
        .find(|track| track.bone == 1 && track.property == Property::Translation)
        .expect("root translation track");
    assert_vec3_near(
        translation
            .key_vec3(translation.key_count() - 1)
            .expect("final translation key"),
        Vec3::Y,
    );

    // Geometry measurements describe the mesh definition's finite POSITION
    // stream. They deliberately do not bake this instance's converted world
    // transform into the result: the transform carries the centimetre-to-metre
    // and Z-up-to-Y-up conversion for this fixture.
    let finite_positions = primitive
        .positions
        .iter()
        .copied()
        .filter(|position| position.is_finite())
        .collect::<Vec<_>>();
    assert!(!finite_positions.is_empty(), "fixture has finite positions");
    let local_centroid =
        finite_positions.iter().copied().sum::<Vec3>() / finite_positions.len() as f32;
    let measured_centroid = measure_assets(&doc).mesh_definitions[0]
        .geometry_centroid
        .expect("finite FBX geometry has a centroid");
    assert_vec3_near(Vec3::from_array(measured_centroid), local_centroid);
    assert!(
        (model.transform_point3(local_centroid) - local_centroid).length() > 1e-5,
        "fixture node transform must make the world centroid observably different"
    );
}
