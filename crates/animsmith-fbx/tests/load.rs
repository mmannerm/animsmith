use animsmith_core::glam::{Mat4, Vec3};
use animsmith_core::measure::measure_assets;
use animsmith_core::model::{
    Property, SourceInverseBindAccessorStatus, SourceNodeLocalRest, SourceSkeletonCoverage,
};
use animsmith_core::scale::{
    ScaleCapabilityCoverage, ScaleError, ScaleOperation, ScaleRequest, plan_scale,
};
use animsmith_core::{Document, MeasurementContract, TrackValues, validate_document_shape};
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
        assert_eq!(
            loaded
                .inventory()
                .empty_source_meshes
                .iter()
                .map(|identity| (identity.source_index, identity.ufbx_typed_id))
                .collect::<Vec<_>>(),
            if label == "empty" {
                vec![(0, 0)]
            } else {
                Vec::new()
            },
            "{label}"
        );
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

    let loaded = animsmith_fbx::load_scale_source(&path).expect("vertex-color fixture parses");
    assert_eq!(loaded.inventory().unsupported_vertex_payload_mesh_count, 1);
    assert_eq!(
        loaded.inventory().domains.other_vertex_and_source_data,
        FbxScaleDomainStatus::Unsupported
    );
    assert!(
        animsmith_fbx::capability_facts(loaded.inventory()).unsupported_vertex_attributes_present
    );
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
fn both_scale_operations_remain_typed_refusals_for_inventory_only_fbx() {
    let source = animsmith_fbx::load_scale_source(&fixture()).expect("fixture loads");
    let facts = animsmith_fbx::capability_facts(source.inventory());
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

    let source = animsmith_fbx::load_scale_source_bytes(&path, &bytes).expect("captured FBX loads");

    assert_eq!(source.inventory().external_resource_count, 2);
    assert!(animsmith_fbx::capability_facts(source.inventory()).external_resources_present);
    assert_normal_texture(source.document());
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
