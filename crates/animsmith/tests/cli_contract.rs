use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::*;
use animsmith_core::{
    Check, CheckCtx, CheckSelection, Config, DependencyClosureBuilderV1, InputIdentity,
    LintEnvelope, LintFileReport, MeasurementContract, MetricGrids, RawMeshPrimitiveRowsV1,
    RawNodeMeshAttachmentRowsV1, RawSceneAttachmentCoverageV1, RawSceneAttachmentInventoryV1,
    RawSceneRootRowV1, RawSceneRootRowsV1, RawSourceFactsBuilderV1, RawSourceSkeletonEvidenceV1,
    ResolvedRoles, RigInfo, SourceFactDomainV1, SourceFormatV1, SourceSkeletonCoverage, ToolInfo,
    ToolSource, evaluate_checks_v2,
};
use animsmith_engine::{
    BevyGltfHandlerEnvironmentV2, BevyLoadMeshesStateV2, EngineDeclarationV2, EngineUnitScaleCheck,
    ProfileSelection, SettingValueV2, project_prediction_provenance_v4, resolve_static_v2,
};
use animsmith_gltf::fix::{FixSession, Repair as GltfRepair};
use animsmith_testkit::{quats_from_angles, scaled_quat, two_bone_rotation_doc};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const CURRENT_OUTPUT_SCHEMA_ID: &str = "urn:animsmith:schema:output:17";
const OUTPUT_V16_SCHEMA_ID: &str = "urn:animsmith:schema:output:16";
const OUTPUT_V14_SCHEMA_ID: &str = "urn:animsmith:schema:output:14";
const OUTPUT_V13_SCHEMA_ID: &str = "urn:animsmith:schema:output:13";
const OUTPUT_V10_SCHEMA_ID: &str = "urn:animsmith:schema:output:10";
const MEASUREMENTS_V15_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:15";
const MEASUREMENTS_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:16";
const ADDRESSABILITY_SCHEMA_ID: &str = "urn:animsmith:schema:gltf-animation-addressability:1";
const IMPORT_ADVICE_SCHEMA_ID: &str = "urn:animsmith:schema:engine-import-advice:1";
const HOSTILE_PRESENTATION_TEXT: &str = "forged\nline\u{1b}[31m\u{2028}\u{2029}\u{202e}";
const CURRENT_OUTPUT_SCHEMA: &str = include_str!("../../../docs/schemas/output-v17.schema.json");
const OUTPUT_V16_SCHEMA: &str = include_str!("../../../docs/schemas/output-v16.schema.json");
const OUTPUT_V15_SCHEMA: &str = include_str!("../../../docs/schemas/output-v15.schema.json");
const OUTPUT_V14_SCHEMA: &str = include_str!("../../../docs/schemas/output-v14.schema.json");
const OUTPUT_V13_SCHEMA: &str = include_str!("../../../docs/schemas/output-v13.schema.json");
const OUTPUT_V10_SCHEMA: &str = include_str!("../../../docs/schemas/output-v10.schema.json");
const MEASUREMENTS_V15_SCHEMA: &str =
    include_str!("../../../docs/schemas/measurements-v15.schema.json");
const MEASUREMENTS_SCHEMA: &str =
    include_str!("../../../docs/schemas/measurements-v16.schema.json");
const ADDRESSABILITY_SCHEMA: &str =
    include_str!("../../../docs/schemas/gltf-animation-addressability-v1.schema.json");
const IMPORT_ADVICE_SCHEMA: &str =
    include_str!("../../../docs/schemas/engine-import-advice-v1.schema.json");
#[cfg(feature = "fbx")]
const RIGGED_TRIANGLE_FBX: &str = include_str!("../../animsmith-fbx/testdata/rigged_triangle.fbx");
const EXPECTED_CHECK_IDS: [&str; 31] = [
    "nan",
    "time-monotonic",
    "quat-norm",
    "quat-flip",
    "duration-sanity",
    "scale-keys",
    "non-uniform-scale",
    "constant-nonunit-scale",
    "constant-track",
    "missing-bones",
    "required-bones",
    "rest-world-scale",
    "frozen-bone",
    "duplicate-loop-endpoint",
    "loop-closure",
    "loop-seam",
    "loop-seam-vel",
    "loop-seam-rot",
    "root-motion-speed",
    "gait-group",
    "sync-group",
    "time-complement",
    "in-place",
    "fps",
    "bind-pose",
    "foot-slide",
    "engine-addressability",
    "engine-clip-boundary",
    "engine-unit-scale",
    "engine-track-support",
    "engine-root-motion",
];

fn output_validator() -> jsonschema::Validator {
    let output: Value =
        serde_json::from_str(CURRENT_OUTPUT_SCHEMA).expect("valid output schema JSON");
    let output_v14: Value =
        serde_json::from_str(OUTPUT_V14_SCHEMA).expect("valid historical output schema JSON");
    let output_v13: Value =
        serde_json::from_str(OUTPUT_V13_SCHEMA).expect("valid historical output schema JSON");
    let output_v10: Value =
        serde_json::from_str(OUTPUT_V10_SCHEMA).expect("valid historical output schema JSON");
    let measurements_v15: Value = serde_json::from_str(MEASUREMENTS_V15_SCHEMA)
        .expect("valid historical measurement schema JSON");
    let measurements: Value =
        serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurement schema JSON");
    let registry = jsonschema::Registry::new()
        .add(OUTPUT_V14_SCHEMA_ID, output_v14)
        .expect("valid historical output-v14 schema identity")
        .add(OUTPUT_V13_SCHEMA_ID, output_v13)
        .expect("valid historical output-v13 schema identity")
        .add(OUTPUT_V10_SCHEMA_ID, output_v10)
        .expect("valid historical output schema identity")
        .add(MEASUREMENTS_V15_SCHEMA_ID, measurements_v15)
        .expect("valid historical measurement schema identity")
        .add(MEASUREMENTS_SCHEMA_ID, measurements)
        .expect("valid measurement schema identity")
        .prepare()
        .expect("measurement schema registry prepares");
    jsonschema::options()
        .with_registry(&registry)
        .build(&output)
        .expect("output schema compiles with nested measurement contract")
}

fn assert_output_schema_valid(instance: &Value) {
    let validator = output_validator();
    let errors: Vec<_> = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "output must satisfy the published v17 schemas:\n{}\ninstance: {instance:#}",
        errors.join("\n")
    );
}

fn output_v16_validator() -> jsonschema::Validator {
    let output: Value =
        serde_json::from_str(OUTPUT_V16_SCHEMA).expect("valid historical output-v16 schema JSON");
    let measurements_v15: Value = serde_json::from_str(MEASUREMENTS_V15_SCHEMA)
        .expect("valid historical measurement schema JSON");
    let measurements: Value =
        serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurement schema JSON");
    let registry = jsonschema::Registry::new()
        .add(MEASUREMENTS_V15_SCHEMA_ID, measurements_v15)
        .expect("valid historical measurement schema identity")
        .add(MEASUREMENTS_SCHEMA_ID, measurements)
        .expect("valid measurement schema identity")
        .prepare()
        .expect("historical measurement schema registry prepares");
    jsonschema::options()
        .with_registry(&registry)
        .build(&output)
        .expect("output-v16 schema compiles with nested measurement contract")
}

fn assert_output_v16_schema_valid(instance: &Value) {
    let errors: Vec<_> = output_v16_validator()
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "output must satisfy the historical v16 schemas:\n{}\ninstance: {instance:#}",
        errors.join("\n")
    );
}

fn assert_output_v15_schema_valid(instance: &Value) {
    let output: Value =
        serde_json::from_str(OUTPUT_V15_SCHEMA).expect("valid output-v15 schema JSON");
    let measurements_v15: Value = serde_json::from_str(MEASUREMENTS_V15_SCHEMA)
        .expect("valid historical measurement schema JSON");
    let measurements: Value =
        serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurement schema JSON");
    let registry = jsonschema::Registry::new()
        .add(MEASUREMENTS_V15_SCHEMA_ID, measurements_v15)
        .expect("valid historical measurement schema identity")
        .add(MEASUREMENTS_SCHEMA_ID, measurements)
        .expect("valid measurement schema identity")
        .prepare()
        .expect("historical measurement schema registry prepares");
    let validator = jsonschema::options()
        .with_registry(&registry)
        .build(&output)
        .expect("output-v15 schema compiles with nested measurement contract");
    let errors: Vec<_> = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "output must satisfy the historical v15 schemas:\n{}\ninstance: {instance:#}",
        errors.join("\n")
    );
}

fn addressability_validator() -> jsonschema::Validator {
    let output: Value =
        serde_json::from_str(OUTPUT_V10_SCHEMA).expect("valid historical output schema JSON");
    let measurements_v15: Value = serde_json::from_str(MEASUREMENTS_V15_SCHEMA)
        .expect("valid historical measurement schema JSON");
    let measurements: Value =
        serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurement schema JSON");
    let addressability: Value =
        serde_json::from_str(ADDRESSABILITY_SCHEMA).expect("valid addressability schema JSON");
    let registry = jsonschema::Registry::new()
        .add(MEASUREMENTS_SCHEMA_ID, measurements)
        .expect("valid measurement schema identity")
        .add(MEASUREMENTS_V15_SCHEMA_ID, measurements_v15)
        .expect("valid historical measurement schema identity")
        .add(OUTPUT_V10_SCHEMA_ID, output)
        .expect("valid output schema identity")
        .prepare()
        .expect("addressability schema registry prepares");
    jsonschema::options()
        .with_registry(&registry)
        .build(&addressability)
        .expect("addressability schema compiles with reused historical output-v10 definitions")
}

fn assert_addressability_schema_valid(instance: &Value) {
    let errors: Vec<_> = addressability_validator()
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "addressability output must satisfy the published V1 schema:\n{}\ninstance: {instance:#}",
        errors.join("\n")
    );
}

fn import_advice_validator() -> jsonschema::Validator {
    let output: Value =
        serde_json::from_str(OUTPUT_V10_SCHEMA).expect("valid historical output schema JSON");
    let measurements_v15: Value = serde_json::from_str(MEASUREMENTS_V15_SCHEMA)
        .expect("valid historical measurement schema JSON");
    let measurements: Value =
        serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurement schema JSON");
    let advice: Value =
        serde_json::from_str(IMPORT_ADVICE_SCHEMA).expect("valid import-advice schema JSON");
    let registry = jsonschema::Registry::new()
        .add(MEASUREMENTS_SCHEMA_ID, measurements)
        .expect("valid measurement schema identity")
        .add(MEASUREMENTS_V15_SCHEMA_ID, measurements_v15)
        .expect("valid historical measurement schema identity")
        .add(OUTPUT_V10_SCHEMA_ID, output)
        .expect("valid output schema identity")
        .prepare()
        .expect("import-advice schema registry prepares");
    jsonschema::options()
        .with_registry(&registry)
        .build(&advice)
        .expect("import-advice schema compiles with reused output definitions")
}

fn assert_import_advice_schema_valid(instance: &Value) {
    let errors: Vec<_> = import_advice_validator()
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "import-advice output must satisfy the published V1 schema:\n{}\ninstance: {instance:#}",
        errors.join("\n")
    );
}

fn assert_evaluation_summary_matches_checks(instance: &Value) {
    let checks: Vec<_> = instance["files"]
        .as_array()
        .expect("output files")
        .iter()
        .flat_map(|file| file["checks"].as_array().expect("output checks"))
        .collect();
    let summary = &instance["summary"]["checks"];
    for (field, dimension, value) in [
        ("complete", "evaluation", "complete"),
        ("partial", "evaluation", "partial"),
        ("not_evaluated", "evaluation", "not_evaluated"),
    ] {
        let expected = checks
            .iter()
            .filter(|check| check[dimension] == value)
            .count();
        assert_eq!(summary["evaluation"][field], expected, "summary.{field}");
    }
    for (field, dimension, value) in [
        ("not_applicable", "applicability", "not_applicable"),
        ("disabled", "configuration", "disabled"),
        ("unselected", "selection", "unselected"),
    ] {
        let expected = checks
            .iter()
            .filter(|check| check[dimension] == value)
            .count();
        assert_eq!(summary[dimension][field], expected, "summary.{field}");
    }
    let expected_gaps: usize = checks
        .iter()
        .map(|check| check["gaps"].as_array().map_or(0, Vec::len))
        .sum();
    assert_eq!(summary["gaps"], expected_gaps, "summary.gaps");
    let total = checks.len();
    assert_eq!(summary["total"], total);
    for fields in [
        &["selected", "unselected"][..],
        &["enabled", "disabled"][..],
        &["applicable", "not_applicable"][..],
        &["complete", "partial", "not_evaluated"][..],
    ] {
        let axis = if fields[0] == "selected" {
            "selection"
        } else if fields[0] == "enabled" {
            "configuration"
        } else if fields[0] == "applicable" {
            "applicability"
        } else {
            "evaluation"
        };
        let sum: u64 = fields
            .iter()
            .map(|field| summary[axis][field].as_u64().unwrap())
            .sum();
        assert_eq!(sum, total as u64, "{axis} partition");
    }
}

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(name)
}

fn example_asset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/assets")
        .join(name)
}

fn unique_temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("animsmith-cli-{name}-"))
        .tempdir()
        .expect("creates temp dir")
}

fn embedded_input_identity() -> animsmith_core::InputIdentity {
    animsmith_core::InputIdentity::from_bytes(b"embedded primary-file bytes")
}

fn input_identity_json(path: &std::path::Path) -> Value {
    serde_json::to_value(animsmith_core::InputIdentity::from_bytes(
        &std::fs::read(path).expect("reads fixture bytes"),
    ))
    .expect("input identity serializes")
}

/// Analytic rotation sequence: consecutive y-rotations 0.4 rad apart,
/// so every adjacent pair has a positive dot product — the clean form
/// is exactly the un-negated sequence.
fn sway_quats(flipped: bool) -> Vec<Quat> {
    let mut quats = quats_from_angles(&[0.0, 0.4, 0.8, 1.2, 1.6]);
    if flipped {
        quats[1] = -quats[1];
        quats[3] = -quats[3];
    }
    quats
}

fn sway_doc_with_quats(quats: Vec<Quat>) -> Document {
    two_bone_rotation_doc("sway", quats, false)
}

fn sway_doc(flipped: bool) -> Document {
    sway_doc_with_quats(sway_quats(flipped))
}

fn root_trajectory_doc(clip_name: &str) -> Document {
    let mut doc = sway_doc(false);
    let quarter_turn = std::f32::consts::FRAC_1_SQRT_2;
    let positive_quarter_turn = Quat::from_xyzw(0.0, quarter_turn, 0.0, quarter_turn);
    let half_turn = Quat::from_xyzw(0.0, 1.0, 0.0, 0.0);
    let negative_quarter_turn = Quat::from_xyzw(0.0, -quarter_turn, 0.0, quarter_turn);
    let times = vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0];
    doc.clips = vec![Clip {
        name: clip_name.into(),
        duration_s: 1.0,
        tracks: vec![
            Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: times.clone(),
                values: TrackValues::Vec3s(vec![
                    Vec3::ZERO,
                    Vec3::new(2.0, -3.0, 0.0),
                    Vec3::new(2.0, 5.0, -4.0),
                    Vec3::new(-1.0, 2.0, -4.0),
                    Vec3::new(-1.0, 2.0, -10.0),
                    Vec3::new(5.0, 2.0, -10.0),
                ]),
            },
            Track {
                bone: 0,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times,
                values: TrackValues::Quats(vec![
                    Quat::IDENTITY,
                    positive_quarter_turn,
                    half_turn,
                    negative_quarter_turn,
                    Quat::IDENTITY,
                    negative_quarter_turn,
                ]),
            },
        ],
    }];
    doc
}

fn write_root_trajectory_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&root_trajectory_doc("trajectory"), path)
        .expect("writes analytic root-trajectory fixture");
}

fn write_named_root_trajectory_glb(path: &std::path::Path, clip_name: &str) {
    animsmith_gltf::write::write(&root_trajectory_doc(clip_name), path)
        .expect("writes named analytic root-trajectory fixture");
}

fn sway_doc_with_distinct_repairs() -> Document {
    let mut quats = sway_quats(true);
    quats[1] = scaled_quat(quats[1], 1.2);
    sway_doc_with_quats(quats)
}

fn write_flipped_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&sway_doc(true), path).expect("writes flipped fixture");
}

fn write_distinct_repair_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&sway_doc_with_distinct_repairs(), path)
        .expect("writes distinct repair fixture");
}

fn write_clean_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&sway_doc(false), path).expect("writes clean fixture");
}

/// A static skeleton with one deliberately ambiguous display name. It has no
/// clips, which proves the required-bones contract is structural rather than
/// a disguised per-clip keyframe requirement.
fn write_required_bones_glb(path: &std::path::Path) {
    let bone = |name: &str| Bone {
        name: name.into(),
        parent: None,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    };
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![
                bone("root"),
                bone("weapon_socket"),
                bone("duplicate"),
                bone("duplicate"),
            ],
        },
        ..Document::default()
    };
    animsmith_gltf::write::write(&doc, path).expect("writes required-bones fixture");
}

fn write_empty_skeleton_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&Document::default(), path)
        .expect("writes empty-skeleton fixture");
}

/// A closed rotational loop with a C1 cusp at the wrap: it leaves at 2 rad/s
/// and enters at -1 rad/s. There are no translation tracks, so the linear
/// seam metric must remain zero while the angular check reports the cusp.
fn write_angular_cusp_glb(path: &std::path::Path) {
    let doc = two_bone_rotation_doc(
        "angular_cusp",
        quats_from_angles(&[0.0, 0.5, 1.0, 0.25, 0.0]),
        false,
    );
    animsmith_gltf::write::write(&doc, path).expect("writes angular-cusp fixture");
}

fn write_two_clip_clean_glb(path: &std::path::Path) {
    let mut doc = sway_doc(false);
    let mut second = doc.clips[0].clone();
    second.name = "sway_b".into();
    doc.clips.push(second);
    animsmith_gltf::write::write(&doc, path).expect("writes two-clip fixture");
}

fn write_time_complement_glb(path: &std::path::Path) {
    let mut doc =
        animsmith_gltf::load(&example_asset("walk.glb")).expect("loads synthetic walk example");
    doc.clips[0].name = "forward".into();
    let mut reflected = doc.clips[0].clone();
    reflected.name = "reflected".into();
    for track in &mut reflected.tracks {
        match &mut track.values {
            TrackValues::Vec3s(values) => values.reverse(),
            TrackValues::Quats(_) => unreachable!("walk fixture uses translation tracks"),
        }
    }
    doc.clips.push(reflected);
    animsmith_gltf::write::write(&doc, path).expect("writes time-complement fixture");
}

fn write_duplicate_loop_endpoint_glb(path: &std::path::Path) {
    let mut doc = sway_doc(false);
    doc.clips[0].name = "guard".into();
    doc.clips[0].duration_s = 1.0;
    doc.clips[0].tracks = vec![Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 0.25, 0.5, 0.75, 1.0],
        values: TrackValues::Vec3s(vec![
            Vec3::ZERO,
            Vec3::X,
            2.0 * Vec3::X,
            Vec3::X,
            Vec3::ZERO,
        ]),
    }];
    animsmith_gltf::write::write(&doc, path).expect("writes duplicate endpoint fixture");
}

fn write_hostile_glb(path: &std::path::Path, hostile: &str, flipped: bool) {
    let mut doc = sway_doc(flipped);
    doc.clips[0].name = hostile.into();
    for bone in &mut doc.skeleton.bones {
        bone.name = hostile.into();
    }
    doc.assets.meshes.push(MeshAsset {
        name: hostile.into(),
        source_mesh_index: doc.assets.meshes.len(),
        primitives: vec![Primitive {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            ..Primitive::default()
        }],
    });
    doc.assets.instances.push(MeshInstance {
        source_node_index: doc.assets.instances.len(),
        node: 0,
        mesh: doc.assets.meshes.len() - 1,
        ..MeshInstance::default()
    });
    animsmith_gltf::write::write(&doc, path).expect("writes hostile-name fixture");
}

fn write_multi_mesh_glb(path: &std::path::Path) {
    let bone = |name: &str| Bone {
        name: name.into(),
        parent: None,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    };
    let material = |name: &str| MaterialAsset {
        name: name.into(),
        base_color: [1.0; 4],
        metallic: 0.0,
        roughness: 1.0,
        base_color_texture: None,
        normal_texture: None,
        metallic_roughness_texture: None,
        occlusion_texture: None,
    };
    let primitive = |material| Primitive {
        material,
        positions: vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ],
        ..Primitive::default()
    };
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![
                bone("body-node"),
                bone("prop-node"),
                bone(" duplicate-node "),
                bone(" duplicate-node "),
                bone(" duplicate-node "),
            ],
        },
        assets: SceneAssets {
            meshes: vec![
                MeshAsset {
                    name: "body-mesh".into(),
                    source_mesh_index: 0,
                    primitives: vec![primitive(Some(0))],
                },
                MeshAsset {
                    name: "prop-mesh".into(),
                    source_mesh_index: 1,
                    primitives: vec![primitive(Some(1))],
                },
            ],
            instances: vec![
                MeshInstance {
                    source_node_index: 0,
                    node: 0,
                    mesh: 0,
                    ..MeshInstance::default()
                },
                MeshInstance {
                    source_node_index: 1,
                    node: 1,
                    mesh: 1,
                    ..MeshInstance::default()
                },
                MeshInstance {
                    source_node_index: 2,
                    node: 2,
                    mesh: 0,
                    ..MeshInstance::default()
                },
                MeshInstance {
                    source_node_index: 3,
                    node: 3,
                    mesh: 0,
                    ..MeshInstance::default()
                },
                MeshInstance {
                    source_node_index: 4,
                    node: 4,
                    mesh: 0,
                    ..MeshInstance::default()
                },
            ],
            materials: vec![
                material("body-material"),
                material("prop-material"),
                material(" duplicate-material "),
                material(" duplicate-material "),
                material(" duplicate-material "),
            ],
            ..SceneAssets::default()
        },
        ..Document::default()
    };
    animsmith_gltf::write::write(&doc, path).expect("writes multi-mesh fixture");
}

fn assert_hostile_text_is_escaped(text: &str) {
    assert!(
        !text.contains(HOSTILE_PRESENTATION_TEXT),
        "raw hostile text leaked:\n{text}"
    );
    assert!(
        !text.contains('\u{1b}'),
        "raw terminal escape leaked:\n{text}"
    );
    assert!(
        !text.contains('\u{2028}'),
        "raw line separator leaked:\n{text}"
    );
    assert!(
        !text.contains('\u{2029}'),
        "raw paragraph separator leaked:\n{text}"
    );
    assert!(
        !text.contains('\u{202e}'),
        "raw bidi override leaked:\n{text}"
    );
    assert!(
        text.contains("forged\\nline\\u{1b}[31m\\u{2028}\\u{2029}\\u{202e}"),
        "escaped form missing:\n{text}"
    );
}

fn write_json(path: &std::path::Path, value: &Value) {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("serializes JSON fixture"),
    )
    .expect("writes JSON fixture");
}

fn write_humanoid_profile_gltf(path: &std::path::Path, prefix: &str) {
    let names = [
        "root".to_owned(),
        format!("{prefix} Pelvis"),
        format!("{prefix} Spine"),
        format!("{prefix} Head"),
        format!("{prefix} L Foot"),
        format!("{prefix} R Foot"),
        format!("{prefix} L Toe0"),
        format!("{prefix} R Toe0"),
        format!("{prefix} L Hand"),
        format!("{prefix} R Hand"),
    ];
    let nodes = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            if index == 0 {
                json!({ "name": name, "children": (1..names.len()).collect::<Vec<_>>() })
            } else {
                json!({ "name": name })
            }
        })
        .collect::<Vec<_>>();
    write_json(
        path,
        &json!({
            "asset": { "version": "2.0" },
            "nodes": nodes,
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }),
    );
}

fn measurement_report(duration_s: f64) -> Value {
    json!({
        "schema_version": 13,
        "schema": OUTPUT_V13_SCHEMA_ID,
        "tool": {
            "name": "animsmith",
            "version": env!("CARGO_PKG_VERSION"),
            "source": { "revision": null, "dirty": null }
        },
        "command": "measure",
        "summary": { "files": 1 },
        "files": [{
            "path": "fixture.gltf",
            "input": {
                "sha256": "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae",
                "bytes": 3
            },
            "rig": {
                "profile": "unknown",
                "resolution_outcome": "coverage",
                "resolved_roles": {},
                "resolved_role_policies": {}
            },
            "measurements": {
                "schema_version": 16,
                "schema": MEASUREMENTS_SCHEMA_ID,
                "clips": {
                    "walk": {
                        "duration_s": duration_s,
                        "frame_count": 31,
                        "animated_bones": [],
                        "bone_channels": [],
                        "bone_rotation_range_deg": {},
                        "loop_continuity_availability": "not_applicable",
                        "loop_endpoint_mode_availability": "not_applicable",
                        "frame_grid_availability": "not_applicable",
                        "loop_seam_ratio_availability": "not_applicable",
                        "gait_availability": "not_applicable",
                        "root_trajectory_availability": "not_applicable",
                        "speed_mps_availability": "not_applicable"
                    }
                },
                "mesh_definitions": [],
                "node_instances": [],
                "scenes": [],
                "material_resource_coverage": "unavailable",
                "material_definitions": [],
                "textures": [],
                "images": [],
                "skeleton_source_coverage": "unavailable",
                "skeleton_nodes": [],
                "skins": []
            }
        }]
    })
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

const EMPTY_ANIMATION_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "nodes": [{ "name": "root" }],
  "animations": [{ "name": "empty", "samplers": [], "channels": [] }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

fn write_source_animation_inventory_gltf(path: &std::path::Path, names: &[Option<&str>]) {
    let animations = names
        .iter()
        .map(|name| {
            let mut animation = serde_json::Map::from_iter([
                ("samplers".to_owned(), json!([])),
                ("channels".to_owned(), json!([])),
            ]);
            if let Some(name) = name {
                animation.insert("name".to_owned(), json!(name));
            }
            Value::Object(animation)
        })
        .collect::<Vec<_>>();
    write_json(
        path,
        &json!({
            "asset": { "version": "2.0" },
            "nodes": [{ "name": "root" }],
            "animations": animations,
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }),
    );
}

fn write_source_animation_channel_overflow_gltf(path: &std::path::Path) {
    // Prime the raw-source text budget with bounded unsupported declarations,
    // then make the one animation's discarded morph-channel prefix cross the
    // remaining budget well before the aggregate row cap. Prediction settings
    // retain only the short clip name, so the adapter can report the semantic
    // required-unavailable result instead of failing provenance materialization.
    let extensions_used = (0..1_536)
        .map(|index| {
            let prefix = format!("X_ANIMSMITH_TEST_{index:04}_");
            format!("{prefix}{}", "x".repeat(4_096 - prefix.len()))
        })
        .collect::<Vec<_>>();
    let channels = (0..32_768)
        .map(|_| json!({ "sampler": 0, "target": { "node": 0, "path": "weights" } }))
        .collect::<Vec<_>>();
    write_json(
        path,
        &json!({
            "asset": { "version": "2.0" },
            "extensionsUsed": extensions_used,
            "buffers": [{
                "uri": "data:application/octet-stream;base64,AAAAAAAAgD8=",
                "byteLength": 8
            }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 8 }],
            "accessors": [
                {
                    "bufferView": 0,
                    "componentType": 5126,
                    "count": 2,
                    "type": "SCALAR",
                    "min": [0.0],
                    "max": [1.0]
                },
                { "bufferView": 0, "componentType": 5126, "count": 2, "type": "SCALAR" }
            ],
            "nodes": [{ "name": "root" }],
            "animations": [{
                "name": "morph",
                "samplers": [{ "input": 0, "output": 1 }],
                "channels": channels
            }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }),
    );
}

fn write_bevy_config(dir: &std::path::Path, suffix: &str) -> PathBuf {
    write_config(
        dir,
        &format!("bevy-{suffix}.toml"),
        r#"
[engine]
profile = "bevy"
profile_revision = 1
engine_version = "0.19.0"
importer = "gltf-asset-loader"
"#,
    )
}

fn write_bevy_v2_config(dir: &std::path::Path, suffix: &str) -> PathBuf {
    write_config(
        dir,
        &format!("bevy-v2-{suffix}.toml"),
        r#"
[engine]
profile = "bevy"
profile_revision = 2
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
bevy_animation_feature = true
"#,
    )
}

fn write_bevy_v3_track_config(
    dir: &std::path::Path,
    suffix: &str,
    bevy_animation_feature: bool,
    load_animations: Option<bool>,
) -> PathBuf {
    let load_animations = load_animations
        .map(|value| format!("\nload_animations = {value}"))
        .unwrap_or_default();
    write_config(
        dir,
        &format!("bevy-v3-track-{suffix}.toml"),
        &format!(
            r#"
[engine]
profile = "bevy"
profile_revision = 3
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
bevy_animation_feature = {bevy_animation_feature}{load_animations}
"#
        ),
    )
}

fn write_track_support_gltf(path: &std::path::Path, channels_per_animation: &[usize]) {
    let animations = channels_per_animation
        .iter()
        .map(|&channel_count| {
            let channels = (0..channel_count)
                .map(|_| json!({ "sampler": 0, "target": { "node": 0, "path": "translation" } }))
                .collect::<Vec<_>>();
            json!({
                "samplers": if channel_count == 0 { vec![] } else { vec![json!({ "input": 0, "output": 1 })] },
                "channels": channels,
            })
        })
        .collect::<Vec<_>>();
    write_json(
        path,
        &json!({
            "asset": { "version": "2.0" },
            "buffers": [{
                "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                "byteLength": 24
            }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 24 }],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0.0], "max": [1.0] },
                { "bufferView": 0, "componentType": 5126, "count": 2, "type": "VEC3" }
            ],
            "nodes": [{ "name": "root" }],
            "animations": animations,
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }),
    );
}

fn track_support_facets(report: &Value) -> &Vec<Value> {
    lint_check(report, "engine-track-support")["prediction"]["prediction"]["facets"]
        .as_array()
        .expect("nested V5 track-support facets")
}

fn lint_check<'a>(json: &'a Value, check_id: &str) -> &'a Value {
    json["files"][0]["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["check_id"] == check_id)
        .unwrap_or_else(|| panic!("missing {check_id} record"))
}

fn lint_check_mut<'a>(json: &'a mut Value, check_id: &str) -> &'a mut Value {
    json["files"][0]["checks"]
        .as_array_mut()
        .expect("checks array")
        .iter_mut()
        .find(|check| check["check_id"] == check_id)
        .unwrap_or_else(|| panic!("missing {check_id} record"))
}

fn synthetic_selected_unit_scale_lint_report(witness: &str, nodes: Vec<SourceNodeAsset>) -> Value {
    // JSON glTF cannot spell a non-finite node transform. Build the same
    // post-loader authority directly, then exercise the real engine producer,
    // Historical output-v15 constructor, serializer, and CLI reader.
    let primary = InputIdentity::from_bytes(witness.as_bytes());
    let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::Glb, primary.clone());
    for domain in [
        SourceFactDomainV1::Clips,
        SourceFactDomainV1::Constructs,
        SourceFactDomainV1::Resources,
    ] {
        facts.mark_complete(domain);
    }
    let closure = DependencyClosureBuilderV1::new(
        primary.clone(),
        facts.resource_coverage(),
        facts.resource_rows().len(),
    )
    .finish()
    .expect("complete synthetic dependency closure");
    let node_count = nodes.len() as u64;
    let mut document = Document::default();
    document.assets.source_skeleton.coverage = SourceSkeletonCoverage::Complete;
    document.assets.source_skeleton.nodes = nodes;
    let source = facts
        .finish_with_dependency_closure(document, closure)
        .expect("valid synthetic loaded source");
    let inventory = RawSceneAttachmentInventoryV1::new(
        primary.clone(),
        RawSourceSkeletonEvidenceV1::new(RawSceneAttachmentCoverageV1::Complete, node_count, 0),
        RawSceneRootRowsV1::new(
            RawSceneAttachmentCoverageV1::Complete,
            vec![RawSceneRootRowV1::new(0, vec![0])],
        ),
        RawNodeMeshAttachmentRowsV1::new(RawSceneAttachmentCoverageV1::Complete, Vec::new()),
        RawMeshPrimitiveRowsV1::new(RawSceneAttachmentCoverageV1::Complete, Vec::new()),
    )
    .expect("valid synthetic raw scene inventory");
    let source = source
        .with_raw_scene_attachment_inventory(inventory)
        .expect("same-load inventory attaches");
    let settings = BTreeMap::from([
        (
            "bevy_animation_feature".into(),
            SettingValueV2::Boolean(true),
        ),
        (
            "extension_handler_environment".into(),
            SettingValueV2::HandlerEnvironment(BevyGltfHandlerEnvironmentV2::BareEmpty),
        ),
        (
            "load_meshes".into(),
            SettingValueV2::LoadMeshesState(BevyLoadMeshesStateV2::Empty),
        ),
        ("rotate_scene_entity".into(), SettingValueV2::Boolean(false)),
    ]);
    let resolved = resolve_static_v2(EngineDeclarationV2 {
        selection: Some(ProfileSelection::new(
            "bevy",
            2,
            "0.19.0",
            "gltf-asset-loader",
        )),
        document_settings: Some(settings),
        ..EngineDeclarationV2::default()
    })
    .expect("valid frozen Bevy profile")
    .expect("profile selected")
    .resolve_input(SourceFormatV1::Glb)
    .expect("GLB is accepted");
    let provenance =
        project_prediction_provenance_v4(&resolved, &source, vec!["Socket".to_owned()])
            .expect("valid same-load V4 provenance");
    let mut config = Config::default();
    config.runtime_nodes.selectors = Some(vec!["Socket".to_owned()]);
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(source.document());
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let check: Box<dyn Check + '_> =
        Box::new(EngineUnitScaleCheck::new(&source, Some(&provenance)).unwrap());
    let evaluations =
        evaluate_checks_v2(&ctx, &[check], CheckSelection::All).expect("synthetic check evaluates");
    let measurements = MeasurementContract::new(
        BTreeMap::new(),
        animsmith_core::measure::measure_assets(source.document()),
    )
    .expect("valid synthetic measurement contract");
    let rig = RigInfo::from_resolved(source.document(), &roles).expect("valid empty synthetic rig");
    let file = LintFileReport::new_v4(
        format!("{witness}.glb"),
        primary,
        rig,
        Some(provenance),
        evaluations,
        measurements,
    )
    .expect("producer-valid V4 lint file");
    serde_json::to_value(
        LintEnvelope::new(ToolInfo::animsmith(ToolSource::new(None, None)), vec![file])
            .expect("producer-valid V4 lint envelope"),
    )
    .expect("serializable V4 lint envelope")
}

#[test]
fn bevy_revision_2_lint_emits_correlated_v5_unit_scale_results() {
    let dir = unique_temp_dir("bevy-v2-unit-scale");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let config = write_bevy_v2_config(dir.path(), "unit-scale");

    let output = animsmith()
        .arg("--config")
        .arg(config)
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(input)
        .output()
        .expect("runs Bevy revision-2 lint");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("lint JSON");
    assert_output_schema_valid(&json);
    assert_eq!(json["schema"], CURRENT_OUTPUT_SCHEMA_ID);
    assert_eq!(
        json["files"][0]["prediction_provenance"]["schema"],
        "urn:animsmith:prediction-provenance:5"
    );
    assert_eq!(
        json["files"][0]["prediction_provenance"]["base"]["rule_inputs"]["runtime_node_selectors"],
        json!([])
    );
    let check = lint_check(&json, "engine-unit-scale");
    assert_eq!(check["selection"], "selected");
    assert_eq!(check["configuration"], "enabled");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "complete");
    assert_eq!(
        check["prediction"]["schema"],
        "urn:animsmith:engine-prediction:5"
    );
    let file_unit = check["prediction"]["prediction"]["facets"]
        .as_array()
        .expect("prediction facets")
        .iter()
        .find(|facet| facet["scope"]["code"] == "engine-unit-scale:file-unit")
        .expect("file-unit facet");
    assert_eq!(file_unit["state"], "available");
    assert_eq!(file_unit["result"]["kind"], "unit_mapping");
    assert_eq!(
        file_unit["result"]["result"]["exact_target_units_per_source_unit"],
        json!({ "numerator": 1, "denominator": 1 })
    );
    assert_eq!(
        file_unit["result"]["result"]["application_world_unit_policy"],
        "unenforced"
    );
}

#[test]
fn bevy_revision_3_keeps_engine_unit_scale_applicable_with_v5_facets() {
    let dir = unique_temp_dir("bevy-v3-unit-scale");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let output = animsmith()
        .arg("--config")
        .arg(write_bevy_v3_track_config(
            dir.path(),
            "unit-scale",
            true,
            None,
        ))
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(input)
        .output()
        .expect("runs Bevy revision-3 unit-scale lint");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("revision-3 unit-scale JSON");
    assert_output_schema_valid(&json);
    assert_eq!(
        json["files"][0]["prediction_provenance"]["base"]["profile"]["selection"]["profile_revision"],
        3
    );
    let check = lint_check(&json, "engine-unit-scale");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "complete");
    let facets = check["prediction"]["prediction"]["facets"]
        .as_array()
        .expect("nested unit-scale facets");
    assert!(facets.iter().any(|facet| {
        facet["scope"]["code"] == "engine-unit-scale:file-unit"
            && facet["state"] == "available"
            && facet["result"]["kind"] == "unit_mapping"
    }));
}

#[test]
fn bevy_revision_2_readback_rejects_active_unit_scale_with_null_v4_provenance() {
    let dir = unique_temp_dir("bevy-v2-unit-scale-null-provenance");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let config = write_bevy_v2_config(dir.path(), "unit-scale-null-provenance");
    let output = animsmith()
        .arg("--config")
        .arg(config)
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(input)
        .output()
        .expect("runs Bevy revision-2 lint");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let mut hostile: Value = serde_json::from_slice(&output.stdout).expect("lint JSON");
    hostile["files"][0]["prediction_provenance"] = Value::Null;
    lint_check_mut(&mut hostile, "engine-unit-scale")
        .as_object_mut()
        .expect("unit-scale check object")
        .remove("prediction");
    assert_output_schema_valid(&hostile);

    let report = dir.path().join("null-provenance.json");
    write_json(&report, &hostile);
    let readback = animsmith()
        .arg("diff")
        .arg(&report)
        .arg(&report)
        .output()
        .expect("reads hostile V16 report");
    assert_eq!(readback.status.code(), Some(2), "{}", stderr(&readback));
    assert!(
        stderr(&readback).contains("prediction"),
        "{}",
        stderr(&readback)
    );
}

#[test]
fn bevy_revision_2_join_work_overflow_round_trips_without_mesh_prefix() {
    let dir = unique_temp_dir("bevy-v2-join-work-overflow");
    let input = dir.path().join("unmatched-deep-join.gltf");
    let nodes = (0..130)
        .map(|index| {
            let mut node = json!({});
            if index < 65 {
                node["mesh"] = json!(0);
            }
            if index < 64 {
                node["children"] = json!([index + 1]);
            }
            node
        })
        .collect::<Vec<_>>();
    let scenes = (0..65)
        .map(|index| json!({ "nodes": [65 + index] }))
        .collect::<Vec<_>>();
    write_json(
        &input,
        &json!({
            "asset": { "version": "2.0" },
            "nodes": nodes,
            "scenes": scenes,
            "scene": 0,
            "buffers": [{
                "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                "byteLength": 36
            }],
            "bufferViews": [{ "buffer": 0, "byteLength": 36 }],
            "accessors": [{
                "bufferView": 0,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3",
                "min": [0.0, 0.0, 0.0],
                "max": [0.0, 0.0, 0.0]
            }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]
        }),
    );
    let config = write_bevy_v2_config(dir.path(), "join-work-overflow");

    let linted = animsmith()
        .arg("--config")
        .arg(config)
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(&input)
        .output()
        .expect("runs join-work-overflow lint");
    assert_eq!(linted.status.code(), Some(1), "{}", stderr(&linted));
    let report: Value = serde_json::from_slice(&linted.stdout).expect("lint JSON");
    assert_output_schema_valid(&report);
    let facets = lint_check(&report, "engine-unit-scale")["prediction"]["prediction"]["facets"]
        .as_array()
        .expect("prediction facets");
    assert_eq!(facets.len(), 67);
    assert!(!facets.iter().any(|facet| {
        facet["scope"]["code"] == "engine-unit-scale:loader-mesh-primitive"
            || facet["scope"]["code"] == "engine-unit-scale:facet-budget"
    }));
    let overflow = facets
        .iter()
        .find(|facet| facet["scope"]["code"] == "engine-unit-scale:mesh-inventory")
        .expect("mesh join-work unavailable facet");
    assert_eq!(overflow["state"], "required_prediction_unavailable");
    assert_eq!(
        overflow["reasons"],
        json!(["animsmith:mesh_join_work_budget_exceeded"])
    );

    let report_path = dir.path().join("join-work-overflow.json");
    write_json(&report_path, &report);
    let readback = animsmith()
        .args(["diff"])
        .arg(&report_path)
        .arg(&report_path)
        .args(["--format", "json"])
        .output()
        .expect("reads back join-work-overflow report");
    assert_eq!(readback.status.code(), Some(0), "{}", stderr(&readback));
}

#[test]
fn bevy_revision_2_selected_unavailable_authored_kinds_round_trip_and_reject_swaps() {
    let dir = unique_temp_dir("bevy-v2-selected-unavailable-kinds");
    let source_node =
        |index: usize, parent: Option<usize>, name: &str, local_rest: SourceNodeLocalRest| {
            let mut node = SourceNodeAsset::new(index, local_rest);
            node.parent_source_node_index = parent;
            node.name = Some(name.to_owned());
            node
        };
    let trs = |scale| SourceNodeLocalRest::Trs {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale,
    };
    let all_trs = synthetic_selected_unit_scale_lint_report(
        "selected-non-finite-all-trs",
        vec![
            source_node(0, None, "Root", trs(Vec3::ONE)),
            source_node(1, Some(0), "Socket", trs(Vec3::new(f32::NAN, 1.0, 1.0))),
        ],
    );
    let mixed_matrix = synthetic_selected_unit_scale_lint_report(
        "selected-non-finite-trs-under-matrix",
        vec![
            source_node(
                0,
                None,
                "Root",
                SourceNodeLocalRest::Matrix(animsmith_core::glam::Mat4::IDENTITY),
            ),
            source_node(1, Some(0), "Socket", trs(Vec3::new(f32::NAN, 1.0, 1.0))),
        ],
    );
    let selected_subject = "selector:Socket:source_scene:0:source_node:1";
    let selected_facet = |report: &Value| {
        lint_check(report, "engine-unit-scale")["prediction"]["facets"]
            .as_array()
            .expect("prediction facets")
            .iter()
            .find(|facet| {
                facet["scope"]["code"] == "engine-unit-scale:selected-source-node"
                    && facet["scope"]["subject"] == selected_subject
            })
            .expect("resolved selected-node facet")
            .clone()
    };
    assert_eq!(
        selected_facet(&all_trs)["reasons"],
        json!(["measurement_unavailable"])
    );
    assert_eq!(
        selected_facet(&mixed_matrix)["reasons"],
        json!(["animsmith:matrix_authored_selected_node_or_ancestry"])
    );

    let run_diff = |name: &str, report: &Value| {
        let path = dir.path().join(name);
        write_json(&path, report);
        animsmith()
            .arg("diff")
            .arg(&path)
            .arg(&path)
            .output()
            .expect("reads synthetic producer lint JSON")
    };
    for (name, report) in [
        ("all-trs.json", &all_trs),
        ("mixed-matrix.json", &mixed_matrix),
    ] {
        assert_output_v15_schema_valid(report);
        let output = run_diff(name, report);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    }

    for (name, source, replacement) in [
        (
            "all-trs-with-matrix-basis.json",
            &all_trs,
            selected_facet(&mixed_matrix)["basis"].clone(),
        ),
        (
            "matrix-with-all-trs-basis.json",
            &mixed_matrix,
            selected_facet(&all_trs)["basis"].clone(),
        ),
    ] {
        let mut hostile = source.clone();
        let facet = lint_check_mut(&mut hostile, "engine-unit-scale")["prediction"]["facets"]
            .as_array_mut()
            .expect("prediction facets")
            .iter_mut()
            .find(|facet| {
                facet["scope"]["code"] == "engine-unit-scale:selected-source-node"
                    && facet["scope"]["subject"] == selected_subject
            })
            .expect("resolved selected-node facet");
        facet["basis"] = replacement;
        let output = run_diff(name, &hostile);
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert!(
            stderr(&output).contains("prediction"),
            "{}",
            stderr(&output)
        );
    }
}

#[test]
fn bevy_revision_2_missing_runtime_selector_is_unsuppressible() {
    let dir = unique_temp_dir("bevy-v2-unit-scale-missing-selector");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let config = write_config(
        dir.path(),
        "bevy-v2-missing.toml",
        r#"
[engine]
profile = "bevy"
profile_revision = 2
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
bevy_animation_feature = true

[runtime_nodes]
selectors = ["missing_socket"]
"#,
    );

    let output = animsmith()
        .arg("--config")
        .arg(config)
        .args([
            "lint",
            "--select",
            "engine-unit-scale",
            "--allow",
            "engine-unit-scale",
        ])
        .arg(input)
        .output()
        .expect("runs Bevy revision-2 lint");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let json_output = animsmith()
        .arg("--config")
        .arg(dir.path().join("bevy-v2-missing.toml"))
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(dir.path().join("sway.glb"))
        .output()
        .expect("runs machine-readable Bevy revision-2 lint");
    assert_eq!(
        json_output.status.code(),
        Some(1),
        "{}",
        stderr(&json_output)
    );
    let json: Value = serde_json::from_slice(&json_output.stdout).expect("lint JSON");
    assert_output_schema_valid(&json);
    assert_eq!(
        json["files"][0]["prediction_provenance"]["base"]["rule_inputs"]["runtime_node_selectors"],
        json!(["missing_socket"])
    );
    let check = lint_check(&json, "engine-unit-scale");
    let missing = check["prediction"]["prediction"]["facets"]
        .as_array()
        .expect("prediction facets")
        .iter()
        .find(|facet| facet["scope"]["subject"] == "selector:missing_socket")
        .expect("missing-selector facet");
    assert!(
        missing["reasons"]
            .as_array()
            .expect("reasons")
            .iter()
            .any(|reason| reason == "source_selector_no_match")
    );
    assert_eq!(missing["state"], "required_prediction_unavailable");
}

#[test]
fn bevy_revision_2_selected_reachability_planning_stops_at_cumulative_n_plus_one() {
    let dir = unique_temp_dir("bevy-v2-selected-reachability-planning");
    let input = dir.path().join("many-selectors.gltf");
    let selectors = (0..128)
        .map(|index| format!("target-{index}"))
        .collect::<Vec<_>>();
    let selector_toml = selectors
        .iter()
        .map(|selector| format!("\"{selector}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let config = write_config(
        dir.path(),
        "many-selectors.toml",
        &format!(
            r#"
[engine]
profile = "bevy"
profile_revision = 2
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
bevy_animation_feature = true

[runtime_nodes]
selectors = [{selector_toml}]
"#
        ),
    );
    write_json(
        &input,
        &json!({
            "asset": { "version": "2.0" },
            "nodes": std::iter::once(json!({
                "name": "root",
                "children": (1..=128).collect::<Vec<_>>()
            }))
            .chain((0..128).map(|index| json!({ "name": format!("target-{index}") })))
            .collect::<Vec<_>>(),
            "scenes": (0..33).map(|_| json!({ "nodes": [0] })).collect::<Vec<_>>(),
            "scene": 0,
        }),
    );
    let output = animsmith()
        .arg("--config")
        .arg(config)
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(&input)
        .output()
        .expect("runs cumulative selected reachability lint");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).expect("lint JSON");
    let facets = lint_check(&report, "engine-unit-scale")["prediction"]["prediction"]["facets"]
        .as_array()
        .expect("prediction facets");
    assert_eq!(facets.len(), 4096, "{facets:#?}");
    let budget_facets = facets
        .iter()
        .filter(|facet| facet["scope"]["code"] == "engine-unit-scale:facet-budget")
        .collect::<Vec<_>>();
    assert_eq!(budget_facets.len(), 1, "{facets:#?}");
    assert_eq!(
        budget_facets[0]["reasons"],
        json!(["facet_budget_exceeded"])
    );
    assert_eq!(
        facets
            .iter()
            .filter(|facet| facet["scope"]["code"] != "engine-unit-scale:facet-budget")
            .count(),
        4095,
        "{facets:#?}"
    );
    let report_path = dir.path().join("many-selectors.json");
    write_json(&report_path, &report);
    let readback = animsmith()
        .arg("diff")
        .arg(&report_path)
        .arg(&report_path)
        .output()
        .expect("reads cumulative selected reachability report");
    assert_eq!(readback.status.code(), Some(0), "{}", stderr(&readback));
}

#[test]
fn bevy_revision_2_unit_scale_readback_rederives_required_facets_results_and_bases() {
    let dir = unique_temp_dir("bevy-v2-unit-scale-readback");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let config = write_bevy_v2_config(dir.path(), "readback");

    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(&input)
        .output()
        .expect("runs Bevy revision-2 lint");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let valid: Value = serde_json::from_slice(&output.stdout).expect("lint JSON");

    let run_diff = |name: &str, report: &Value| {
        let path = dir.path().join(name);
        write_json(&path, report);
        animsmith()
            .arg("diff")
            .arg(&path)
            .arg(&path)
            .output()
            .expect("runs report diff")
    };

    let mut missing_file_facet = valid.clone();
    let check = lint_check_mut(&mut missing_file_facet, "engine-unit-scale");
    check["prediction"]["prediction"]["facets"]
        .as_array_mut()
        .expect("prediction facets")
        .retain(|facet| facet["scope"]["code"] != "engine-unit-scale:file-unit");
    check["evaluated_scopes"]
        .as_array_mut()
        .expect("evaluated scopes")
        .retain(|scope| scope["code"] != "engine-unit-scale:file-unit");
    missing_file_facet["summary"]["prediction_facets"]["available"] = json!(2);
    let output = run_diff("missing-file-facet.json", &missing_file_facet);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("prediction"),
        "{}",
        stderr(&output)
    );

    let mut forged_result = valid.clone();
    let check = lint_check_mut(&mut forged_result, "engine-unit-scale");
    let scene_facet = check["prediction"]["prediction"]["facets"]
        .as_array_mut()
        .expect("prediction facets")
        .iter_mut()
        .find(|facet| facet["scope"]["code"] == "engine-unit-scale:loader-scene-root")
        .expect("loader-scene-root facet");
    scene_facet["result"]["result"]["subject_kind"] = json!("loader_mesh_primitive");
    let output = run_diff("forged-result.json", &forged_result);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("prediction"),
        "{}",
        stderr(&output)
    );

    let selector_config = write_config(
        dir.path(),
        "bevy-v2-readback-selector.toml",
        r#"
[engine]
profile = "bevy"
profile_revision = 2
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
bevy_animation_feature = true

[runtime_nodes]
selectors = ["missing_socket"]
"#,
    );
    let selector_output = animsmith()
        .arg("--config")
        .arg(selector_config)
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(input)
        .output()
        .expect("runs missing-selector lint");
    assert_eq!(
        selector_output.status.code(),
        Some(1),
        "{}",
        stderr(&selector_output)
    );
    let mut substituted_basis: Value =
        serde_json::from_slice(&selector_output.stdout).expect("selector lint JSON");
    let check = lint_check_mut(&mut substituted_basis, "engine-unit-scale");
    let facets = check["prediction"]["prediction"]["facets"]
        .as_array_mut()
        .expect("prediction facets");
    let available_basis = facets
        .iter()
        .find(|facet| facet["scope"]["code"] == "engine-unit-scale:file-unit")
        .expect("file-unit facet")["basis"]
        .clone();
    let unavailable = facets
        .iter_mut()
        .find(|facet| facet["state"] == "required_prediction_unavailable")
        .expect("unavailable selector facet");
    unavailable["basis"] = available_basis;
    let output = run_diff("substituted-unavailable-basis.json", &substituted_basis);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("prediction"),
        "{}",
        stderr(&output)
    );

    let multi_scene_input = dir.path().join("two-scenes.gltf");
    write_json(
        &multi_scene_input,
        &json!({
            "asset": { "version": "2.0" },
            "nodes": [{ "name": "root" }],
            "scenes": [{ "nodes": [0] }, { "nodes": [0] }],
            "scene": 0
        }),
    );
    let multi_scene_config = write_config(
        dir.path(),
        "bevy-v2-readback-scenes.toml",
        r#"
[engine]
profile = "bevy"
profile_revision = 2
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
bevy_animation_feature = true
"#,
    );
    let output = animsmith()
        .arg("--config")
        .arg(multi_scene_config)
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(multi_scene_input)
        .output()
        .expect("runs multi-scene Bevy revision-2 lint");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let multi_scene: Value = serde_json::from_slice(&output.stdout).expect("multi-scene lint JSON");

    let (name, scope_code) = (
        "swapped-scene-bases.json",
        "engine-unit-scale:loader-scene-root",
    );
    {
        let mut swapped = multi_scene.clone();
        let facets = lint_check_mut(&mut swapped, "engine-unit-scale")["prediction"]["prediction"]
            ["facets"]
            .as_array_mut()
            .expect("prediction facets");
        let indices = facets
            .iter()
            .enumerate()
            .filter_map(|(index, facet)| (facet["scope"]["code"] == scope_code).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(
            indices.len(),
            2,
            "fixture must expose two {scope_code} facets"
        );
        let left = facets[indices[0]]["basis"].clone();
        facets[indices[0]]["basis"] = facets[indices[1]]["basis"].clone();
        facets[indices[1]]["basis"] = left;
        let output = run_diff(name, &swapped);
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert!(
            stderr(&output).contains("prediction"),
            "{}",
            stderr(&output)
        );
    }

    let multi_mesh_input = dir.path().join("multi-mesh.glb");
    write_multi_mesh_glb(&multi_mesh_input);
    let multi_mesh_config = write_config(
        dir.path(),
        "bevy-v2-readback-meshes.toml",
        r#"
[engine]
profile = "bevy"
profile_revision = 2
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
bevy_animation_feature = true

[runtime_nodes]
selectors = ["body-node", "prop-node"]
"#,
    );
    let multi_mesh_output = animsmith()
        .arg("--config")
        .arg(multi_mesh_config)
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(multi_mesh_input)
        .output()
        .expect("runs multi-mesh Bevy revision-2 lint");
    assert_eq!(
        multi_mesh_output.status.code(),
        Some(0),
        "{}",
        stderr(&multi_mesh_output)
    );
    let multi_mesh: Value =
        serde_json::from_slice(&multi_mesh_output.stdout).expect("multi-mesh lint JSON");
    for (name, scope_code) in [
        (
            "swapped-mesh-bases.json",
            "engine-unit-scale:loader-mesh-primitive",
        ),
        (
            "swapped-selected-bases.json",
            "engine-unit-scale:selected-source-node",
        ),
    ] {
        let mut swapped = multi_mesh.clone();
        let facets = lint_check_mut(&mut swapped, "engine-unit-scale")["prediction"]["prediction"]
            ["facets"]
            .as_array_mut()
            .expect("prediction facets");
        let indices = facets
            .iter()
            .enumerate()
            .filter_map(|(index, facet)| (facet["scope"]["code"] == scope_code).then_some(index))
            .collect::<Vec<_>>();
        assert!(
            indices.len() >= 2,
            "fixture must expose multiple {scope_code} facets"
        );
        let left = facets[indices[0]]["basis"].clone();
        facets[indices[0]]["basis"] = facets[indices[1]]["basis"].clone();
        facets[indices[1]]["basis"] = left;
        let output = run_diff(name, &swapped);
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert!(
            stderr(&output).contains("prediction"),
            "{}",
            stderr(&output)
        );
    }

    let raw_selector_config = write_config(
        dir.path(),
        "bevy-v2-readback-raw-selector.toml",
        r#"
[engine]
profile = "bevy"
profile_revision = 2
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
bevy_animation_feature = true

[runtime_nodes]
selectors = ["target"]
"#,
    );
    let run_unreachable_selector = |name: &str, nodes: Value| {
        let path = dir.path().join(format!("{name}.gltf"));
        write_json(
            &path,
            &json!({
                "asset": { "version": "2.0" },
                "nodes": nodes,
                "scenes": [{ "nodes": [] }],
                "scene": 0
            }),
        );
        let output = animsmith()
            .arg("--config")
            .arg(&raw_selector_config)
            .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
            .arg(path)
            .output()
            .expect("runs unreachable-selector unit-scale lint");
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        serde_json::from_slice::<Value>(&output.stdout).expect("unreachable-selector lint JSON")
    };
    let trs_unreachable = run_unreachable_selector(
        "selector-trs-parent-zero",
        json!([
            { "name": "root", "children": [1] },
            { "name": "target" }
        ]),
    );
    let matrix_unreachable = run_unreachable_selector(
        "selector-matrix-parent-zero",
        json!([
            { "name": "root", "children": [1] },
            {
                "name": "target",
                "matrix": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
                           0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
            }
        ]),
    );
    let alternate_parent = run_unreachable_selector(
        "selector-trs-parent-two",
        json!([
            { "name": "root-a" },
            { "name": "target" },
            { "name": "root-b", "children": [1] }
        ]),
    );
    let no_match = run_unreachable_selector("selector-no-match", json!([{ "name": "other" }]));
    let ambiguous = run_unreachable_selector(
        "selector-ambiguous",
        json!([{ "name": "target" }, { "name": "target" }]),
    );
    let selected_basis = |report: &Value, subject: &str| {
        lint_check(report, "engine-unit-scale")["prediction"]["prediction"]["facets"]
            .as_array()
            .expect("prediction facets")
            .iter()
            .find(|facet| {
                facet["scope"]["code"] == "engine-unit-scale:selected-source-node"
                    && facet["scope"]["subject"] == subject
            })
            .expect("selector-only facet")["basis"]
            .clone()
    };
    for (name, source_report, replacement_basis) in [
        (
            "omitted-selected-raw-source-basis.json",
            &trs_unreachable,
            selected_basis(&no_match, "selector:target"),
        ),
        (
            "omitted-ambiguous-name-witnesses.json",
            &ambiguous,
            selected_basis(&no_match, "selector:target"),
        ),
        (
            "substituted-selected-local-kind-basis.json",
            &matrix_unreachable,
            selected_basis(&trs_unreachable, "selector:target"),
        ),
        (
            "substituted-selected-parent-basis.json",
            &trs_unreachable,
            selected_basis(&alternate_parent, "selector:target"),
        ),
    ] {
        let mut hostile = source_report.clone();
        let facet =
            lint_check_mut(&mut hostile, "engine-unit-scale")["prediction"]["prediction"]["facets"]
                .as_array_mut()
                .expect("prediction facets")
                .iter_mut()
                .find(|facet| {
                    facet["scope"]["code"] == "engine-unit-scale:selected-source-node"
                        && facet["scope"]["subject"] == "selector:target"
                })
                .expect("selector-only facet");
        facet["basis"] = replacement_basis;
        let output = run_diff(name, &hostile);
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert!(
            stderr(&output).contains("prediction"),
            "{}",
            stderr(&output)
        );
    }

    let deep_nodes = (0..129)
        .map(|index| {
            let mut node = serde_json::Map::new();
            node.insert(
                "name".to_owned(),
                json!(if index == 128 { "target" } else { "chain" }),
            );
            if index < 128 {
                node.insert("children".to_owned(), json!([index + 1]));
            }
            Value::Object(node)
        })
        .collect::<Vec<_>>();
    let bounded_nodes = (0..128)
        .map(|index| {
            let mut node = serde_json::Map::new();
            node.insert(
                "name".to_owned(),
                json!(if index == 127 { "target" } else { "chain" }),
            );
            if index < 127 {
                node.insert("children".to_owned(), json!([index + 1]));
            }
            Value::Object(node)
        })
        .collect::<Vec<_>>();
    let run_reachable_selector = |name: &str, nodes: Vec<Value>, expected_status: i32| {
        let path = dir.path().join(format!("{name}.gltf"));
        write_json(
            &path,
            &json!({
                "asset": { "version": "2.0" },
                "nodes": nodes,
                "scenes": [{ "nodes": [0] }],
                "scene": 0
            }),
        );
        let output = animsmith()
            .arg("--config")
            .arg(&raw_selector_config)
            .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
            .arg(path)
            .output()
            .expect("runs reachable-selector unit-scale lint");
        assert_eq!(
            output.status.code(),
            Some(expected_status),
            "{}",
            stderr(&output)
        );
        serde_json::from_slice::<Value>(&output.stdout).expect("reachable-selector lint JSON")
    };
    let deep_ancestry = run_reachable_selector("selector-deep-ancestry", deep_nodes, 1);
    let bounded_ancestry = run_reachable_selector("selector-bounded-ancestry", bounded_nodes, 0);
    let selected_facets =
        lint_check(&deep_ancestry, "engine-unit-scale")["prediction"]["prediction"]["facets"]
            .as_array()
            .expect("prediction facets");
    let deep_facet = selected_facets
        .iter()
        .find(|facet| {
            facet["scope"]["code"] == "engine-unit-scale:selected-source-node"
                && facet["scope"]["subject"] == "selector:target"
        })
        .expect("over-bound reachability has one selector-only facet");
    assert_eq!(
        deep_facet["reasons"],
        json!(["animsmith:selected_node_scene_reachability_unavailable"])
    );
    assert!(!selected_facets.iter().any(|facet| {
        facet["scope"]["subject"] == "selector:target:source_scene:0:source_node:128"
    }));
    assert!(
        lint_check(&bounded_ancestry, "engine-unit-scale")["prediction"]["prediction"]["facets"]
            .as_array()
            .expect("prediction facets")
            .iter()
            .any(|facet| {
                facet["scope"]["subject"] == "selector:target:source_scene:0:source_node:127"
                    && facet["state"] == "available"
            })
    );

    let mut substituted_reachability = deep_ancestry.clone();
    let facet =
        lint_check_mut(&mut substituted_reachability, "engine-unit-scale")["prediction"]["prediction"]["facets"]
            .as_array_mut()
            .expect("prediction facets")
            .iter_mut()
            .find(|facet| {
                facet["scope"]["code"] == "engine-unit-scale:selected-source-node"
                    && facet["scope"]["subject"] == "selector:target"
            })
            .expect("over-bound selected-node facet");
    facet["basis"] = selected_basis(&trs_unreachable, "selector:target");
    let output = run_diff(
        "substituted-selected-reachability-basis.json",
        &substituted_reachability,
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("prediction"),
        "{}",
        stderr(&output)
    );

    let aggregate_nodes = (0..128)
        .map(|index| {
            let mut node = serde_json::Map::new();
            node.insert(
                "name".to_owned(),
                json!(if index == 127 { "target" } else { "chain" }),
            );
            if index < 127 {
                node.insert("children".to_owned(), json!([index + 1]));
            }
            Value::Object(node)
        })
        .collect::<Vec<_>>();
    let run_aggregate_selector = |name: &str, scene_count: u64, expected_status: i32| {
        let path = dir.path().join(format!("{name}.gltf"));
        write_json(
            &path,
            &json!({
                "asset": { "version": "2.0" },
                "nodes": aggregate_nodes.clone(),
                "scenes": (0..scene_count).map(|_| json!({ "nodes": [0] })).collect::<Vec<_>>(),
                "scene": 0
            }),
        );
        let output = animsmith()
            .arg("--config")
            .arg(&raw_selector_config)
            .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
            .arg(path)
            .output()
            .expect("runs aggregate-reachability lint");
        assert_eq!(
            output.status.code(),
            Some(expected_status),
            "{}",
            stderr(&output)
        );
        serde_json::from_slice::<Value>(&output.stdout).expect("aggregate-reachability lint JSON")
    };
    let aggregate_at_limit = run_aggregate_selector("selector-aggregate-at-limit", 32, 0);
    let aggregate_over_limit = run_aggregate_selector("selector-aggregate-over-limit", 33, 1);
    let at_limit_selected =
        lint_check(&aggregate_at_limit, "engine-unit-scale")["prediction"]["prediction"]["facets"]
            .as_array()
            .expect("prediction facets")
            .iter()
            .filter(|facet| facet["scope"]["code"] == "engine-unit-scale:selected-source-node")
            .collect::<Vec<_>>();
    assert_eq!(at_limit_selected.len(), 32);
    assert!(
        at_limit_selected
            .iter()
            .all(|facet| facet["state"] == "available")
    );
    let unreachable_scene_path = dir.path().join("selector-existing-unreachable.gltf");
    write_json(
        &unreachable_scene_path,
        &json!({
            "asset": { "version": "2.0" },
            "nodes": [
                { "name": "root", "children": [1] },
                { "name": "target" },
                { "name": "detached" }
            ],
            "scenes": [{ "nodes": [0] }, { "nodes": [2] }],
            "scene": 0,
        }),
    );
    let output = animsmith()
        .arg("--config")
        .arg(&raw_selector_config)
        .args(["lint", "--select", "engine-unit-scale", "--format", "json"])
        .arg(&unreachable_scene_path)
        .output()
        .expect("runs existing-unreachable-scene lint");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let mut missing_witness: Value = serde_json::from_slice(&output.stdout).expect("lint JSON");
    let facet = lint_check_mut(&mut missing_witness, "engine-unit-scale")["prediction"]["prediction"]["facets"]
        .as_array_mut()
        .expect("prediction facets")
        .iter_mut()
        .find(|facet| {
            facet["scope"]["code"] == "engine-unit-scale:selected-source-node"
                && facet["scope"]["subject"] == "selector:target:source_scene:0:source_node:1"
        })
        .expect("cached selected witness facet");
    facet["scope"]["subject"] = json!("selector:target:source_scene:1:source_node:1");
    fn mutate_scene_witness(value: &mut Value) {
        match value {
            Value::Array(values) => {
                for value in values {
                    mutate_scene_witness(value);
                }
            }
            Value::Object(values) => {
                if values.get("source_scene_index") == Some(&json!(0)) {
                    values.insert("source_scene_index".to_owned(), json!(1));
                }
                if values.contains_key("source_root_ordinal") {
                    values.insert("source_node_index".to_owned(), json!(2));
                }
                for value in values.values_mut() {
                    mutate_scene_witness(value);
                }
            }
            _ => {}
        }
    }
    mutate_scene_witness(&mut facet["basis"]);
    let output = run_diff("forged-selected-missing-witness.json", &missing_witness);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let aggregate_facet =
        lint_check(&aggregate_over_limit, "engine-unit-scale")["prediction"]["prediction"]["facets"]
            .as_array()
            .expect("prediction facets")
            .iter()
            .find(|facet| {
                facet["scope"]["code"] == "engine-unit-scale:selected-source-node"
                    && facet["scope"]["subject"] == "selector:target"
            })
            .expect("aggregate overflow selector-only facet");
    assert_eq!(
        aggregate_facet["reasons"],
        json!(["animsmith:selected_node_scene_reachability_unavailable"])
    );
    let mut substituted_aggregate = aggregate_over_limit.clone();
    let facet =
        lint_check_mut(&mut substituted_aggregate, "engine-unit-scale")["prediction"]["prediction"]
            ["facets"]
            .as_array_mut()
            .expect("prediction facets")
            .iter_mut()
            .find(|facet| {
                facet["scope"]["code"] == "engine-unit-scale:selected-source-node"
                    && facet["scope"]["subject"] == "selector:target"
            })
            .expect("aggregate overflow selector-only facet");
    facet["basis"] = selected_basis(&trs_unreachable, "selector:target");
    let output = run_diff(
        "substituted-selected-aggregate-reachability-basis.json",
        &substituted_aggregate,
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("prediction"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn generate_addressability_is_schema_valid_profile_neutral_and_reuses_the_lint_check_bytes() {
    let dir = unique_temp_dir("generate-addressability-profiles");
    let input = dir.path().join("animations.gltf");
    write_source_animation_inventory_gltf(&input, &[Some("duplicate"), None, Some("duplicate")]);
    let bevy_config = write_bevy_config(dir.path(), "generate");
    let godot_config = write_config(
        dir.path(),
        "godot.toml",
        r#"
[engine]
profile = "godot"
profile_revision = 1
engine_version = "4.7"
importer = "resource-importer-scene"
"#,
    );

    let run = |config: Option<&std::path::Path>| {
        let mut command = animsmith();
        if let Some(config) = config {
            command.arg("--config").arg(config);
        }
        command
            .args(["generate", "addressability"])
            .arg(&input)
            .output()
            .expect("runs generate addressability")
    };

    let neutral = run(None);
    assert_eq!(neutral.status.code(), Some(0), "{}", stderr(&neutral));
    assert!(stderr(&neutral).is_empty());
    let neutral_json: Value = serde_json::from_slice(&neutral.stdout).expect("canonical JSON");
    assert_addressability_schema_valid(&neutral_json);
    assert_eq!(neutral_json["schema_version"], 1);
    assert_eq!(neutral_json["schema"], ADDRESSABILITY_SCHEMA_ID);
    assert_eq!(neutral_json["command"], "generate-addressability");
    assert_eq!(neutral_json["input"], input_identity_json(&input));
    assert_eq!(
        neutral_json["input"],
        neutral_json["inventory"]["primary_input"]
    );
    assert_eq!(neutral_json["inventory"]["source_format"], "gltf_json");
    assert_eq!(
        neutral_json["inventory"]["animations"]["coverage"]["state"],
        "complete"
    );
    assert_eq!(
        neutral_json["inventory"]["animations"]["rows"][0]["source_name"]["value"],
        "duplicate"
    );
    assert_eq!(
        neutral_json["inventory"]["animations"]["rows"][1]["source_name"]["state"],
        "proven_absent"
    );
    assert!(neutral_json["bevy"].is_null());
    let neutral_readback =
        animsmith_engine::GltfAnimationAddressabilityInput::read_from(neutral.stdout.as_slice())
            .expect("strict root read")
            .into_report()
            .expect("strict neutral readback");
    assert!(neutral_readback.bevy().is_none());

    let godot = run(Some(&godot_config));
    assert_eq!(godot.status.code(), Some(0), "{}", stderr(&godot));
    let godot_json: Value = serde_json::from_slice(&godot.stdout).expect("Godot JSON");
    assert_addressability_schema_valid(&godot_json);
    assert!(godot_json["bevy"].is_null());
    assert_eq!(godot_json["inventory"], neutral_json["inventory"]);

    let bevy = run(Some(&bevy_config));
    assert_eq!(bevy.status.code(), Some(0), "{}", stderr(&bevy));
    let bevy_json: Value = serde_json::from_slice(&bevy.stdout).expect("Bevy JSON");
    assert_addressability_schema_valid(&bevy_json);
    assert_eq!(bevy_json["inventory"], neutral_json["inventory"]);
    let readback =
        animsmith_engine::GltfAnimationAddressabilityInput::read_from(bevy.stdout.as_slice())
            .expect("strict root read")
            .into_report()
            .expect("strict Bevy readback");
    let readback_check = readback.bevy().expect("exact Bevy adapter").check();
    assert_eq!(
        readback_check.selection(),
        animsmith_core::SelectionState::Selected
    );
    assert_eq!(
        readback_check.configuration(),
        animsmith_core::ConfigurationState::Enabled
    );
    assert_eq!(
        readback_check.applicability(),
        animsmith_core::Applicability::Applicable
    );
    assert_eq!(
        readback_check.evaluation(),
        animsmith_core::EvaluationState::Complete
    );
    assert_eq!(readback_check.evaluated_scopes().len(), 3);

    let lint = animsmith()
        .arg("--config")
        .arg(&bevy_config)
        .args([
            "lint",
            "--select",
            "engine-addressability",
            "--format",
            "json",
        ])
        .arg(&input)
        .output()
        .expect("runs the original #154 check path");
    assert_eq!(lint.status.code(), Some(0), "{}", stderr(&lint));
    let lint_json: Value = serde_json::from_slice(&lint.stdout).expect("lint JSON");
    let lint_addressability = lint_check(&lint_json, "engine-addressability");
    // Standalone Bevy remains V1 while current lint is the V3
    // bounded-overflow contract; lifecycle evidence stays comparable but the
    // prediction/provenance payloads intentionally do not share bytes.
    assert_eq!(
        bevy_json["bevy"]["check"]["check_id"],
        lint_addressability["check_id"]
    );
    assert_eq!(
        bevy_json["bevy"]["check"]["evaluation"],
        lint_addressability["evaluation"]
    );
    assert_eq!(
        lint_addressability["prediction"]["schema"],
        "urn:animsmith:engine-prediction:3"
    );
    assert_eq!(
        lint_json["files"][0]["prediction_provenance"]["schema"],
        "urn:animsmith:prediction-provenance:3"
    );

    let disabled_config = write_config(
        dir.path(),
        "bevy-disabled.toml",
        r#"
[engine]
profile = "bevy"
profile_revision = 1
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[checks.engine-addressability]
severity = "off"
"#,
    );
    let disabled = run(Some(&disabled_config));
    assert_eq!(disabled.status.code(), Some(0), "{}", stderr(&disabled));
    let disabled_json: Value =
        serde_json::from_slice(&disabled.stdout).expect("disabled adapter JSON");
    assert_addressability_schema_valid(&disabled_json);
    assert_eq!(disabled_json["inventory"], neutral_json["inventory"]);
    assert_eq!(disabled_json["bevy"]["check"]["configuration"], "disabled");
    assert!(disabled_json["bevy"]["check"].get("prediction").is_none());
    let disabled_readback =
        animsmith_engine::GltfAnimationAddressabilityInput::read_from(disabled.stdout.as_slice())
            .expect("strict disabled root read")
            .into_report()
            .expect("strict disabled adapter readback");
    let disabled_check = disabled_readback
        .bevy()
        .expect("disabled exact Bevy adapter remains present")
        .check();
    assert_eq!(
        disabled_check.configuration(),
        animsmith_core::ConfigurationState::Disabled
    );
    assert_eq!(
        disabled_check.evaluation(),
        animsmith_core::EvaluationState::NotEvaluated
    );
    assert!(disabled_check.evaluated_scopes().is_empty());
    let disabled_lint = animsmith()
        .arg("--config")
        .arg(&disabled_config)
        .args([
            "lint",
            "--select",
            "engine-addressability",
            "--format",
            "json",
        ])
        .arg(&input)
        .output()
        .expect("runs disabled #154 check path");
    assert_eq!(disabled_lint.status.code(), Some(0));
    let disabled_lint_json: Value =
        serde_json::from_slice(&disabled_lint.stdout).expect("disabled lint JSON");
    assert_eq!(
        serde_json::to_vec(&disabled_json["bevy"]["check"]).unwrap(),
        serde_json::to_vec(lint_check(&disabled_lint_json, "engine-addressability")).unwrap()
    );

    let mut unknown_root_field = neutral_json.clone();
    unknown_root_field["unknown"] = json!(true);
    assert!(
        !addressability_validator().is_valid(&unknown_root_field),
        "the public schema must reject unknown root fields"
    );
    assert!(
        serde_json::from_value::<animsmith_engine::GltfAnimationAddressabilityInput>(
            unknown_root_field
        )
        .is_err(),
        "the strict reader must reject the same mutation"
    );

    let mut missing_bevy = neutral_json.clone();
    missing_bevy.as_object_mut().unwrap().remove("bevy");
    assert!(!addressability_validator().is_valid(&missing_bevy));
    let missing_bevy: animsmith_engine::GltfAnimationAddressabilityInput =
        serde_json::from_value(missing_bevy).expect("stages a missing required-nullable field");
    assert!(matches!(
        missing_bevy.into_report(),
        Err(animsmith_engine::GltfAnimationAddressabilityError::MissingBevyField)
    ));

    let mut noncanonical_row = neutral_json.clone();
    noncanonical_row["inventory"]["animations"]["rows"][1]["source_clip_index"] = json!(7);
    let noncanonical_row: animsmith_engine::GltfAnimationAddressabilityInput =
        serde_json::from_value(noncanonical_row).expect("stages nested inventory semantics");
    assert!(matches!(
        noncanonical_row.into_report(),
        Err(animsmith_engine::GltfAnimationAddressabilityError::InvalidInventoryShape { .. })
    ));

    let mut invalid_coverage_reason = neutral_json.clone();
    invalid_coverage_reason["inventory"]["animations"]["coverage"] = json!({
        "state": "partial",
        "reason": "not_a_reason"
    });
    assert!(!addressability_validator().is_valid(&invalid_coverage_reason));
    let invalid_coverage_reason: animsmith_engine::GltfAnimationAddressabilityInput =
        serde_json::from_value(invalid_coverage_reason).expect("stages raw nested inventory JSON");
    assert!(matches!(
        invalid_coverage_reason.into_report(),
        Err(animsmith_engine::GltfAnimationAddressabilityError::InvalidInventoryShape { .. })
    ));

    let valid_channel = json!({
        "source_channel_index": 0,
        "target": { "kind": "node", "index": 0 },
        "property": "translation",
        "input_accessor_index": 0,
        "output_accessor_index": 1
    });

    let mut invalid_target_kind = neutral_json.clone();
    invalid_target_kind["inventory"]["animations"]["rows"][0]["channels"]["rows"] =
        json!([valid_channel.clone()]);
    invalid_target_kind["inventory"]["animations"]["rows"][0]["channels"]["rows"][0]["target"]["kind"] =
        json!("element");
    assert!(!addressability_validator().is_valid(&invalid_target_kind));
    let invalid_target_kind: animsmith_engine::GltfAnimationAddressabilityInput =
        serde_json::from_value(invalid_target_kind).expect("stages a non-glTF channel target");
    assert!(matches!(
        invalid_target_kind.into_report(),
        Err(animsmith_engine::GltfAnimationAddressabilityError::InvalidInventoryShape { .. })
    ));

    let mut missing_accessor = neutral_json.clone();
    missing_accessor["inventory"]["animations"]["rows"][0]["channels"]["rows"] =
        json!([valid_channel]);
    missing_accessor["inventory"]["animations"]["rows"][0]["channels"]["rows"][0]["input_accessor_index"] =
        Value::Null;
    assert!(!addressability_validator().is_valid(&missing_accessor));
    let missing_accessor: animsmith_engine::GltfAnimationAddressabilityInput =
        serde_json::from_value(missing_accessor).expect("stages a missing accessor pair member");
    assert!(matches!(
        missing_accessor.into_report(),
        Err(animsmith_engine::GltfAnimationAddressabilityError::InvalidInventoryShape { .. })
    ));

    let mut malformed_check = bevy_json.clone();
    malformed_check["bevy"]["check"]["check_id"] = json!("another-check");
    assert!(!addressability_validator().is_valid(&malformed_check));
    let malformed_check: animsmith_engine::GltfAnimationAddressabilityInput =
        serde_json::from_value(malformed_check).expect("stages raw embedded check JSON");
    assert!(matches!(
        malformed_check.into_report(),
        Err(animsmith_engine::GltfAnimationAddressabilityError::InvalidBevyCheckSubset)
    ));

    let mut malformed_provenance = bevy_json.clone();
    malformed_provenance["bevy"]["prediction_provenance"]["profile"]["selection"]["family"] =
        json!("other");
    assert!(!addressability_validator().is_valid(&malformed_provenance));
    let malformed_provenance: animsmith_engine::GltfAnimationAddressabilityInput =
        serde_json::from_value(malformed_provenance).expect("stages raw provenance JSON");
    assert!(matches!(
        malformed_provenance.into_report(),
        Err(animsmith_engine::GltfAnimationAddressabilityError::InvalidBevyProvenance { .. })
    ));

    let mut reduced_available_basis = bevy_json.clone();
    let references =
        reduced_available_basis["bevy"]["check"]["prediction"]["facets"][0]["basis"]["references"]
            .as_array_mut()
            .expect("available facet basis references");
    references.pop().expect("three-reference #154 basis");
    let references: Vec<animsmith_core::PredictionBasisReferenceV1> =
        serde_json::from_value(Value::Array(references.clone()))
            .expect("remaining references are structurally valid");
    let reduced_basis = animsmith_core::EnginePredictionBasisV1::new(references)
        .expect("reduced basis is internally canonical");
    reduced_available_basis["bevy"]["check"]["prediction"]["facets"][0]["basis"] =
        serde_json::to_value(reduced_basis).expect("serializes reduced basis");
    assert!(
        addressability_validator().is_valid(&reduced_available_basis),
        "the generic schema permits a structurally valid reduced basis"
    );
    let reduced_available_basis: animsmith_engine::GltfAnimationAddressabilityInput =
        serde_json::from_value(reduced_available_basis)
            .expect("stages a structurally valid reduced available basis");
    assert!(matches!(
        reduced_available_basis.into_report(),
        Err(animsmith_engine::GltfAnimationAddressabilityError::InvalidBevyCheckSubset)
    ));

    let mut malformed_profile_identity = bevy_json;
    malformed_profile_identity["bevy"]["prediction_provenance"]["profile"]["identity"]["sha256"] =
        json!("0000000000000000000000000000000000000000000000000000000000000000");
    assert!(!addressability_validator().is_valid(&malformed_profile_identity));
    let malformed_profile_identity: animsmith_engine::GltfAnimationAddressabilityInput =
        serde_json::from_value(malformed_profile_identity)
            .expect("stages a wrong embedded profile identity");
    assert!(matches!(
        malformed_profile_identity.into_report(),
        Err(animsmith_engine::GltfAnimationAddressabilityError::InvalidBevyProvenance { .. })
    ));
}

#[test]
fn generate_addressability_text_and_markdown_render_the_same_bounded_observations() {
    let dir = unique_temp_dir("generate-addressability-renderers");
    let input = dir.path().join("animations.gltf");
    write_source_animation_inventory_gltf(
        &input,
        &[Some("duplicate"), None, Some(HOSTILE_PRESENTATION_TEXT)],
    );
    let config = write_bevy_config(dir.path(), "renderers");

    let json = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["generate", "addressability"])
        .arg(&input)
        .output()
        .expect("renders canonical addressability JSON");
    assert_eq!(json.status.code(), Some(0), "{}", stderr(&json));
    assert!(stderr(&json).is_empty());
    let json: Value = serde_json::from_slice(&json.stdout).expect("canonical addressability JSON");
    assert_addressability_schema_valid(&json);
    assert_eq!(
        json["inventory"]["dependency_closure"]["coverage"]["state"],
        "complete"
    );
    let input_sha256 = json["input"]["sha256"].as_str().expect("input digest");
    let input_bytes = json["input"]["bytes"].as_u64().expect("input bytes");
    let inventory_sha256 = json["inventory"]["identity"]["sha256"]
        .as_str()
        .expect("inventory digest");
    let inventory_bytes = json["inventory"]["identity"]["bytes"]
        .as_u64()
        .expect("inventory canonical bytes");
    let closure_references = json["inventory"]["dependency_closure"]["references"]
        .as_array()
        .expect("closure references")
        .len();
    let external_resources = json["inventory"]["dependency_closure"]["external_resources"]
        .as_array()
        .expect("external resources")
        .len();

    let text = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["generate", "addressability"])
        .arg(&input)
        .args(["--format", "text"])
        .output()
        .expect("renders addressability text");
    assert_eq!(text.status.code(), Some(0), "{}", stderr(&text));
    let text = stdout(&text);
    assert_eq!(
        text,
        format!(
            concat!(
                "glTF animation addressability v1\n",
                "input: sha256={input_sha256} bytes={input_bytes}\n",
                "inventory: sha256={inventory_sha256} canonical-bytes={inventory_bytes}\n",
                "source format: gltf_json\n",
                "dependency closure: complete ({closure_references} reference(s), {external_resources} external resource(s))\n",
                "animations: complete (3 retained row(s))\n",
                "  animation 0: name=\"duplicate\" normalized_clip_index=0 channels=complete (0 retained row(s))\n",
                "  animation 1: name=proven_absent normalized_clip_index=1 channels=complete (0 retained row(s))\n",
                "  animation 2: name=\"forged\\nline\\u001B[31m\\u2028\\u2029\\u202E\" normalized_clip_index=2 channels=complete (0 retained row(s))\n",
                "Bevy adapter: bevy revision 1 (0.19.0 / gltf-asset-loader)\n",
                "  check engine-addressability: selected / enabled / applicable / complete\n",
                "    facet animation_asset_label subject Animation0: available\n",
                "    facet animation_asset_label subject Animation1: available\n",
                "    facet animation_asset_label subject Animation2: available\n",
            ),
            input_sha256 = input_sha256,
            input_bytes = input_bytes,
            inventory_sha256 = inventory_sha256,
            inventory_bytes = inventory_bytes,
            closure_references = closure_references,
            external_resources = external_resources,
        ),
        "text must remain an exact presentation of the validated JSON fields"
    );
    assert!(!text.contains(HOSTILE_PRESENTATION_TEXT), "{text}");

    let markdown = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["generate", "addressability"])
        .arg(&input)
        .args(["--format", "markdown"])
        .output()
        .expect("renders addressability Markdown");
    assert_eq!(markdown.status.code(), Some(0), "{}", stderr(&markdown));
    let markdown = stdout(&markdown);
    assert_eq!(
        markdown,
        format!(
            concat!(
                "# glTF animation addressability v1\n\n",
                "- Input: `{input_sha256}` (`{input_bytes}` bytes)\n",
                "- Inventory: `{inventory_sha256}` (`{inventory_bytes}` canonical bytes)\n",
                "- Source format: `gltf_json`\n",
                "- Dependency closure: `complete` (`{closure_references}` references, `{external_resources}` external resources)\n",
                "- Animations: `complete` (`3` retained rows)\n\n",
                "| Animation | Source name | Normalized clip | Channel coverage | Channels |\n",
                "| ---: | --- | ---: | --- | ---: |\n",
                "| 0 | `\"duplicate\"` | `0` | `complete` | 0 |\n",
                "| 1 | `proven_absent` | `1` | `complete` | 0 |\n",
                "| 2 | `\"forged\\\\nline\\\\u001B[31m\\\\u2028\\\\u2029\\\\u202E\"` | `2` | `complete` | 0 |\n\n",
                "## Bevy adapter\n\n",
                "Profile: `bevy` revision `1` (`0.19.0` / `gltf-asset-loader`).\n\n",
                "Check `engine-addressability`: `selected` / `enabled` / `applicable` / `complete`.\n\n",
                "| Scope | Subject | Prediction | Reasons |\n",
                "| --- | --- | --- | --- |\n",
                "| `animation_asset_label` | `Animation0` | `available` | `—` |\n",
                "| `animation_asset_label` | `Animation1` | `available` | `—` |\n",
                "| `animation_asset_label` | `Animation2` | `available` | `—` |\n",
            ),
            input_sha256 = input_sha256,
            input_bytes = input_bytes,
            inventory_sha256 = inventory_sha256,
            inventory_bytes = inventory_bytes,
            closure_references = closure_references,
            external_resources = external_resources,
        ),
        "Markdown must remain an exact presentation of the validated JSON fields"
    );
    assert!(!markdown.contains(HOSTILE_PRESENTATION_TEXT), "{markdown}");
}

#[test]
fn generate_addressability_operator_errors_are_stderr_only_and_config_precedes_input_io() {
    let dir = unique_temp_dir("generate-addressability-errors");
    let unsupported = dir.path().join("asset.txt");
    std::fs::write(&unsupported, b"not an asset").unwrap();
    let output = animsmith()
        .args(["generate", "addressability"])
        .arg(&unsupported)
        .output()
        .expect("rejects unsupported input");
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains("unsupported input"),
        "{}",
        stderr(&output)
    );

    let bad_config = write_config(
        dir.path(),
        "unknown-profile.toml",
        r#"
[engine]
profile = "bevy-next"
profile_revision = 1
engine_version = "0.19.0"
importer = "gltf-asset-loader"
"#,
    );
    let missing = dir.path().join("missing.glb");
    let output = animsmith()
        .arg("--config")
        .arg(&bad_config)
        .args(["generate", "addressability"])
        .arg(&missing)
        .output()
        .expect("rejects profile before input I/O");
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains("unknown engine profile"),
        "{}",
        stderr(&output)
    );
    assert!(
        !stderr(&output).contains("failed to read"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn generate_addressability_required_unavailable_bevy_inventory_exits_one_without_prefix_labels() {
    let dir = unique_temp_dir("generate-addressability-required-unavailable");
    let input = dir.path().join("partial.gltf");
    write_source_animation_channel_overflow_gltf(&input);
    let config = write_bevy_config(dir.path(), "required-unavailable");

    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["generate", "addressability"])
        .arg(&input)
        .output()
        .expect("runs partial-inventory generation");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).expect("partial inventory JSON");
    assert_addressability_schema_valid(&json);
    assert_eq!(
        json["inventory"]["animations"]["coverage"]["state"],
        "partial"
    );
    let facets = json["bevy"]["check"]["prediction"]["facets"]
        .as_array()
        .expect("required-unavailable facets");
    assert_eq!(facets.len(), 1);
    assert_eq!(
        facets[0]["scope"]["code"],
        "animation_asset_label_inventory"
    );
    assert_eq!(facets[0]["state"], "required_prediction_unavailable");
    assert!(
        !output
            .stdout
            .windows(b"Animation0".len())
            .any(|window| window == b"Animation0"),
        "partial inventories must not emit an authoritative label prefix"
    );

    let mut wrong_partial_reason = json;
    wrong_partial_reason["bevy"]["check"]["prediction"]["facets"][0]["reasons"][0] =
        json!("dependency_closure_incomplete");
    assert!(
        addressability_validator().is_valid(&wrong_partial_reason),
        "the generic schema permits another typed prediction reason"
    );
    let wrong_partial_reason: animsmith_engine::GltfAnimationAddressabilityInput =
        serde_json::from_value(wrong_partial_reason)
            .expect("stages a structurally valid wrong partial reason");
    assert!(matches!(
        wrong_partial_reason.into_report(),
        Err(animsmith_engine::GltfAnimationAddressabilityError::InvalidBevyCheckSubset)
    ));
}

#[test]
fn generate_addressability_complete_empty_inventory_keeps_exact_bevy_check_not_applicable() {
    let dir = unique_temp_dir("generate-addressability-empty");
    let input = dir.path().join("empty.gltf");
    write_source_animation_inventory_gltf(&input, &[]);
    let config = write_bevy_config(dir.path(), "empty-generate");

    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["generate", "addressability"])
        .arg(&input)
        .output()
        .expect("generates complete empty addressability inventory");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stderr(&output).is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).expect("empty inventory JSON");
    assert_addressability_schema_valid(&json);
    assert_eq!(
        json["inventory"]["animations"],
        json!({"coverage":{"state":"complete"},"rows":[]})
    );
    assert_eq!(json["bevy"]["check"]["applicability"], "not_applicable");
    assert_eq!(json["bevy"]["check"]["evaluation"], "not_evaluated");
    assert!(json["bevy"]["check"].get("prediction").is_none());
    let readback =
        animsmith_engine::GltfAnimationAddressabilityInput::read_from(output.stdout.as_slice())
            .unwrap()
            .into_report()
            .expect("strict empty readback");
    assert_eq!(
        readback.bevy().expect("exact adapter").check().evaluation(),
        animsmith_core::EvaluationState::NotEvaluated
    );
}

#[test]
fn generate_import_advice_refusal_is_schema_valid_strict_and_exit_one() {
    let dir = unique_temp_dir("generate-import-advice-godot");
    let input = dir.path().join("animation.gltf");
    write_source_animation_inventory_gltf(&input, &[Some("walk")]);
    let config = write_config(
        dir.path(),
        "godot-import-advice.toml",
        r#"
[engine]
profile = "godot"
profile_revision = 1
engine_version = "4.7"
importer = "resource-importer-scene"
"#,
    );

    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["generate", "import-advice"])
        .arg(&input)
        .output()
        .expect("runs Godot import advice");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).expect("import advice JSON");
    assert_import_advice_schema_valid(&json);
    assert_eq!(json["schema"], IMPORT_ADVICE_SCHEMA_ID);
    assert_eq!(json["command"], "generate-import-advice");
    assert_eq!(json["state"], "refused");
    assert_eq!(json["refusal_reason"], "profile_settings_unmodeled");
    assert_eq!(json["payload"], json!({"engine":"godot"}));
    assert_eq!(json["clips"], json!([]));
    let readback = animsmith_engine::EngineImportAdviceInput::read_from(output.stdout.as_slice())
        .unwrap()
        .into_report()
        .expect("strict import advice readback");
    assert_eq!(
        readback.state(),
        animsmith_engine::EngineImportAdviceStateV1::Refused
    );

    let mut unknown = json.clone();
    unknown["unknown"] = json!(true);
    assert!(!import_advice_validator().is_valid(&unknown));
    assert!(serde_json::from_value::<animsmith_engine::EngineImportAdviceInput>(unknown).is_err());
    assert!(
        serde_json::from_value::<animsmith_engine::GltfAnimationAddressabilityInput>(json).is_err(),
        "standalone V1 roots must reject one another"
    );
}

#[test]
fn generate_import_advice_requires_profile_before_input_io_and_rejects_bevy() {
    let dir = unique_temp_dir("generate-import-advice-errors");
    let missing = dir.path().join("missing.gltf");
    let no_profile = animsmith()
        .args(["generate", "import-advice"])
        .arg(&missing)
        .output()
        .expect("checks profile before input I/O");
    assert_eq!(no_profile.status.code(), Some(2));
    assert!(no_profile.stdout.is_empty());
    assert!(
        stderr(&no_profile).contains("requires a complete [engine] selection and settings"),
        "{}",
        stderr(&no_profile)
    );
    assert!(!stderr(&no_profile).contains("cannot read"));

    let bevy = write_bevy_config(dir.path(), "import-advice");
    let unsupported = animsmith()
        .arg("--config")
        .arg(bevy)
        .args(["generate", "import-advice"])
        .arg(&missing)
        .output()
        .expect("rejects unsupported Bevy advice before input I/O");
    assert_eq!(unsupported.status.code(), Some(2));
    assert!(unsupported.stdout.is_empty());
    assert!(
        stderr(&unsupported).contains("requires an exact Unity, Unreal, or Godot V1 profile"),
        "{}",
        stderr(&unsupported)
    );
    assert!(!stderr(&unsupported).contains("cannot read"));
}

#[cfg(feature = "fbx")]
#[test]
fn generate_import_advice_projects_unity_settings_and_renderer_views() {
    let dir = unique_temp_dir("generate-import-advice-unity");
    let input = dir.path().join("rigged-triangle.fbx");
    std::fs::write(&input, RIGGED_TRIANGLE_FBX).unwrap();
    let config = write_config(
        dir.path(),
        "unity-import-advice.toml",
        r#"
[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
convert_units = true
bake_axis_conversion = false
root_motion_source = "Reference/Root"

[clips."*"]
loop = false
movement_owner_xz = "animation"
movement_owner_y = "gameplay"
movement_owner_yaw = "animation"

[clips."*".engine_settings]
root_rotation = "extract"
root_position_y = "bake"
root_position_xz = "extract"
"#,
    );
    let run = |format: &str| {
        animsmith()
            .arg("--config")
            .arg(&config)
            .args(["generate", "import-advice"])
            .arg(&input)
            .args(["--format", format])
            .output()
            .expect("runs Unity import advice")
    };

    let output = run("json");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stderr(&output).is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).expect("Unity advice JSON");
    assert_import_advice_schema_valid(&json);
    assert_eq!(json["state"], "available");
    assert!(json.get("refusal_reason").is_none());
    assert_eq!(json["clips"].as_array().unwrap().len(), 1);
    assert_eq!(json["clips"][0]["source_clip_index"], 0);
    assert_eq!(json["clips"][0]["normalized_clip_index"], 0);
    assert_eq!(json["clips"][0]["evidence"]["loop"], false);
    assert_eq!(
        json["clips"][0]["evidence"]["movement_owner_xz"],
        "animation"
    );
    assert_eq!(json["payload"]["engine"], "unity-generic");
    assert_eq!(json["payload"]["document"]["convert_units"], true);
    assert_eq!(json["payload"]["document"]["bake_axis_conversion"], false);
    assert_eq!(
        json["payload"]["document"]["root_motion_source"],
        "Reference/Root"
    );
    assert_eq!(json["payload"]["clips"][0]["lock_root_rotation"], false);
    assert_eq!(json["payload"]["clips"][0]["lock_root_height_y"], true);
    assert_eq!(json["payload"]["clips"][0]["lock_root_position_xz"], false);
    let advice_sha256 = json["identity"]["sha256"].as_str().expect("advice SHA-256");
    let advice_identity_bytes = json["identity"]["bytes"]
        .as_u64()
        .expect("advice canonical byte count");

    let semantic_result = |value: &Value| {
        let bytes = serde_json::to_vec(value).unwrap();
        animsmith_engine::EngineImportAdviceInput::read_from(bytes.as_slice())
            .unwrap()
            .into_report()
    };

    let mut explicit_null = json.clone();
    explicit_null["clips"][0]["evidence"]["speed_mps"] = Value::Null;
    assert!(!import_advice_validator().is_valid(&explicit_null));
    assert!(
        serde_json::from_value::<animsmith_engine::EngineImportAdviceInput>(explicit_null).is_err()
    );

    let mut lifecycle = json.clone();
    lifecycle["refusal_reason"] = json!("measurement_unavailable");
    assert!(!import_advice_validator().is_valid(&lifecycle));
    assert!(matches!(
        semantic_result(&lifecycle),
        Err(animsmith_engine::EngineImportAdviceError::InvalidLifecycle)
    ));

    let mut measurement = json.clone();
    measurement["clips"][0]["evidence"]["speed_mps"] = json!(1.0);
    assert!(!import_advice_validator().is_valid(&measurement));
    assert!(matches!(
        semantic_result(&measurement),
        Err(animsmith_engine::EngineImportAdviceError::InvalidMeasurement)
    ));

    let mut identity = json.clone();
    identity["identity"]["bytes"] = json!(1);
    assert_import_advice_schema_valid(&identity);
    assert!(matches!(
        semantic_result(&identity),
        Err(animsmith_engine::EngineImportAdviceError::IdentityMismatch)
    ));

    let mut source_index = json.clone();
    source_index["clips"][0]["source_clip_index"] = json!(1);
    assert_import_advice_schema_valid(&source_index);
    assert!(matches!(
        semantic_result(&source_index),
        Err(animsmith_engine::EngineImportAdviceError::InvalidClipIdentity)
    ));

    let mut profile_identity = json.clone();
    profile_identity["prediction_provenance"]["profile"]["identity"]["bytes"] = json!(1);
    assert_import_advice_schema_valid(&profile_identity);
    assert!(matches!(
        semantic_result(&profile_identity),
        Err(animsmith_engine::EngineImportAdviceError::InvalidProvenance(_))
    ));

    let text = run("text");
    assert_eq!(text.status.code(), Some(0), "{}", stderr(&text));
    let text = stdout(&text);
    assert_eq!(
        text,
        format!(
            concat!(
                "engine import advice v1\n",
                "identity: sha256={} canonical-bytes={}\n",
                "profile: unity-generic revision 1 (6000.3 / fbx-model-importer)\n",
                "state: available\n",
                "clip 0 -> 0 \"take\": source-name=\"take\" duration-s=1 loop=false movement-xz=animation movement-y=gameplay movement-yaw=animation speed=not_applicable loop-endpoint=not_applicable frame-grid=not_applicable\n",
                "Unity document: convert-units=true bake-axis-conversion=false root-motion-source=\"Reference/Root\"\n",
                "Unity clip 0: lock-root-rotation=false lock-root-height-y=true lock-root-position-xz=false\n",
            ),
            advice_sha256, advice_identity_bytes,
        )
    );

    let markdown = run("markdown");
    assert_eq!(markdown.status.code(), Some(0), "{}", stderr(&markdown));
    let markdown = stdout(&markdown);
    assert_eq!(
        markdown,
        format!(
            concat!(
                "# Engine import advice v1\n\n",
                "- Identity: `{}` (`{}` canonical bytes)\n",
                "- Profile: `unity-generic` revision `1` (`6000.3` / `fbx-model-importer`)\n",
                "- State: `available`\n\n",
                "## Clips\n\n",
                "- `0` -> `0` `take`; source name `\"take\"`; duration `1` s; loop `false`; movement XZ/Y/yaw `animation` / `gameplay` / `animation`; speed `not_applicable`; loop endpoint `not_applicable`; frame grid `not_applicable`\n\n",
                "## Importer settings\n\n",
                "- Convert Units: `true`\n",
                "- Bake Axis Conversion: `false`\n",
                "- Root Motion Source: `Reference/Root`\n",
                "- Clip `0`: lock root rotation `false`, height Y `true`, position XZ `false`\n",
            ),
            advice_sha256, advice_identity_bytes,
        )
    );

    let hostile =
        RIGGED_TRIANGLE_FBX.replace("AnimStack::take", "AnimStack::<img src=x onerror=alert(1)>");
    std::fs::write(&input, hostile).unwrap();
    let hostile_markdown = run("markdown");
    assert_eq!(
        hostile_markdown.status.code(),
        Some(0),
        "{}",
        stderr(&hostile_markdown)
    );
    let hostile_markdown = stdout(&hostile_markdown);
    assert!(hostile_markdown.contains(
        "`<img src=x onerror=alert(1)>`; source name `\"<img src=x onerror=alert(1)>\"`"
    ));
    assert!(!hostile_markdown.contains("source name \"<img"));
}

#[test]
fn transform_summary_reports_a_loaded_clip_omitted_from_the_artifact() {
    let dir = unique_temp_dir("transform-empty-animation");
    let input = dir.path().join("empty-animation.gltf");
    let output_path = dir.path().join("transformed.glb");
    std::fs::write(&input, EMPTY_ANIMATION_GLTF).expect("writes empty animation fixture");

    let output = animsmith()
        .arg("transform")
        .arg(&input)
        .arg("-o")
        .arg(&output_path)
        .output()
        .expect("runs transform");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let written = animsmith_gltf::load(&output_path).expect("loads transformed output");
    assert!(written.clips.is_empty(), "empty animation is not emitted");
    assert_eq!(
        stdout(&output),
        format!(
            "wrote {} (1 node(s), 0 clip(s), 0 mesh(es) / 0 position(s), 0 material(s)); dropped 1 clip(s) with no writable tracks\n",
            output_path.display()
        )
    );
}

#[test]
fn duplicate_loop_endpoint_cli_detects_trims_and_exposes_changed_contracts() {
    let dir = unique_temp_dir("duplicate-loop-endpoint");
    let input = dir.path().join("input.glb");
    let output_path = dir.path().join("open-cycle.glb");
    let second_output = dir.path().join("open-cycle-again.glb");
    let undeclared_output = dir.path().join("undeclared.glb");
    let config = dir.path().join("animsmith.toml");
    write_duplicate_loop_endpoint_glb(&input);
    std::fs::write(
        &config,
        "[clips.guard]\nloop = true\nduration_s = { value = 1.0, tolerance = 0.0 }\n",
    )
    .expect("writes loop contract");

    let undeclared = animsmith()
        .arg("transform")
        .arg(&input)
        .arg("-o")
        .arg(&undeclared_output)
        .arg("--drop-duplicate-loop-endpoint")
        .output()
        .expect("runs transform without a loop declaration");
    assert_eq!(undeclared.status.code(), Some(0));
    assert!(
        stdout(&undeclared).contains(
            "duplicate-loop-endpoint skipped 'guard': clip is not declared `loop = true` in config"
        ),
        "stdout:\n{}",
        stdout(&undeclared)
    );
    let undeclared_written =
        animsmith_gltf::load(&undeclared_output).expect("loads unchanged undeclared output");
    assert_eq!(undeclared_written.clips[0].duration_s, 1.0);
    assert_eq!(undeclared_written.clips[0].tracks[0].key_count(), 5);

    let lint = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            input.to_str().unwrap(),
            "--select",
            "duplicate-loop-endpoint",
        ])
        .output()
        .expect("runs duplicate endpoint check");
    assert_eq!(lint.status.code(), Some(0), "stderr:\n{}", stderr(&lint));
    assert!(
        stdout(&lint).contains("warning[duplicate-loop-endpoint]"),
        "stdout:\n{}",
        stdout(&lint)
    );

    let lint_json = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            input.to_str().unwrap(),
            "--select",
            "duplicate-loop-endpoint",
            "--format",
            "json",
        ])
        .output()
        .expect("serializes duplicate endpoint evidence");
    assert_eq!(lint_json.status.code(), Some(0));
    let lint_json: Value = serde_json::from_slice(&lint_json.stdout).expect("valid lint JSON");
    assert_output_schema_valid(&lint_json);
    assert_eq!(lint_json["schema_version"], 17);
    assert_eq!(lint_json["schema"], CURRENT_OUTPUT_SCHEMA_ID);
    assert_eq!(lint_json["files"][0]["measurements"]["schema_version"], 16);
    assert_eq!(
        lint_json["files"][0]["measurements"]["schema"],
        MEASUREMENTS_SCHEMA_ID
    );
    let duplicate_record = lint_json["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["check_id"] == "duplicate-loop-endpoint")
        .expect("duplicate endpoint check record");
    assert_eq!(duplicate_record["evaluation"], "complete");
    assert_eq!(
        duplicate_record["evaluated_scopes"][0]["code"],
        "duplicate_loop_endpoint"
    );
    assert_eq!(duplicate_record["findings"][0]["severity"], "warning");

    let contract_before = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            input.to_str().unwrap(),
            "--select",
            "loop-closure,duration-sanity",
        ])
        .output()
        .expect("checks contracts before trimming");
    assert_eq!(
        contract_before.status.code(),
        Some(0),
        "the candidate starts with clean #14/#15 evidence:\n{}",
        stdout(&contract_before)
    );

    let transformed = animsmith()
        .arg("--config")
        .arg(&config)
        .arg("transform")
        .arg(&input)
        .arg("-o")
        .arg(&output_path)
        .arg("--drop-duplicate-loop-endpoint")
        .output()
        .expect("runs duplicate endpoint transform");
    assert_eq!(
        transformed.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&transformed)
    );
    assert!(
        stdout(&transformed).contains(
            "dropped duplicate loop endpoint 'guard': 1 key(s) per track, duration 1.000000s -> 0.750000s (open cycle)"
        ),
        "stdout:\n{}",
        stdout(&transformed)
    );
    let written = animsmith_gltf::load(&output_path).expect("loads open-cycle output");
    assert_eq!(written.clips[0].duration_s, 0.75);
    assert_eq!(written.clips[0].tracks[0].times, vec![0.0, 0.25, 0.5, 0.75]);

    let duplicate_recheck = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            output_path.to_str().unwrap(),
            "--select",
            "duplicate-loop-endpoint",
        ])
        .output()
        .expect("reruns duplicate endpoint check");
    assert_eq!(
        duplicate_recheck.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&duplicate_recheck)
    );
    assert!(
        !stdout(&duplicate_recheck).contains("warning[duplicate-loop-endpoint]"),
        "stdout:\n{}",
        stdout(&duplicate_recheck)
    );

    // The transform intentionally changes the clip to #22's future
    // open/unique-cycle representation. The inclusive #14 closure contract
    // and the old #15 duration pin must therefore be rerun and reconciled.
    let contract_recheck = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            output_path.to_str().unwrap(),
            "--select",
            "loop-closure,duration-sanity",
        ])
        .output()
        .expect("reruns changed contracts");
    assert_eq!(contract_recheck.status.code(), Some(1));
    let contract_text = stdout(&contract_recheck);
    assert!(
        contract_text.contains(
            "error[loop-closure] clip 'guard' bone 'spine' @0.750s: loop does not close in position"
        ),
        "stdout:\n{contract_text}\nstderr:\n{}",
        stderr(&contract_recheck)
    );
    assert!(
        contract_text.contains(
            "error[duration-sanity] clip 'guard': measured duration 0.7500s disagrees with the declared 1.0000"
        ),
        "stdout:\n{contract_text}\nstderr:\n{}",
        stderr(&contract_recheck)
    );

    let idempotent = animsmith()
        .arg("--config")
        .arg(&config)
        .arg("transform")
        .arg(&output_path)
        .arg("-o")
        .arg(&second_output)
        .arg("--drop-duplicate-loop-endpoint")
        .output()
        .expect("reruns transform");
    assert_eq!(idempotent.status.code(), Some(0));
    assert!(
        stdout(&idempotent).contains(
            "duplicate-loop-endpoint skipped 'guard': no mechanically removable repeated endpoint"
        ),
        "stdout:\n{}",
        stdout(&idempotent)
    );
}

#[test]
fn fix_rejects_unknown_repair_ids() {
    // Nonexistent input on purpose: flag validation must produce exit 2
    // regardless of file state, so no fixture is needed.
    let output = animsmith()
        .args(["fix", "clip.glb", "--dry-run", "--repair", "no-such-repair"])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("quat-flip"),
        "stderr should list valid repair ids:\n{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("quat-norm"),
        "stderr should list valid repair ids:\n{}",
        stderr(&output)
    );
}

#[test]
fn fix_rejects_removed_group_flags() {
    // `--group` and `--list-repairs` were removed in the pre-publish
    // contract trim; wrapper scripts still passing them must fail
    // loudly, not silently change meaning.
    for removed in [&["--group", "default"][..], &["--list-repairs"][..]] {
        let output = animsmith()
            .args(["fix", "clip.glb"])
            .args(removed)
            .output()
            .expect("runs animsmith");

        assert_eq!(
            output.status.code(),
            Some(2),
            "{removed:?} must be rejected; stdout:\n{}",
            stdout(&output)
        );
        assert!(
            stderr(&output).contains("unexpected argument"),
            "stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn fix_requires_an_explicit_write_target() {
    let output = animsmith()
        .args(["fix", "clip.glb"])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("fix requires --output <PATH> or --in-place"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn fix_dry_run_reports_without_writing() {
    let dir = unique_temp_dir("fix-dry-run");
    let input = dir.path().join("dirty.glb");
    write_flipped_glb(&input);
    let before = std::fs::read(&input).expect("reads input");

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
            "--repair",
            "quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    // Pending repairs are findings: dry run exits 1 (the check mode),
    // and the input is untouched.
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("would be fixed"),
        "stdout:\n{}",
        stdout(&output)
    );
    assert_eq!(before, std::fs::read(&input).expect("reads input"));
}

#[test]
fn fix_dry_run_dedupes_duplicate_repairs() {
    let dir = unique_temp_dir("fix-dry-run-dedup");
    let input = dir.path().join("dirty.glb");
    write_flipped_glb(&input);

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
            "--repair",
            "quat-flip,quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);
    assert!(
        out.contains("2 key(s) would be fixed across 1 track(s)"),
        "stdout:\n{out}"
    );
    assert_eq!(
        out.matches("key(s) would be fixed across").count(),
        1,
        "duplicate repairs should be reported once:\n{out}"
    );
}

#[test]
fn fix_dry_run_dedupes_non_adjacent_distinct_repairs_without_writing() {
    let dir = unique_temp_dir("fix-dry-run-compose");
    let input = dir.path().join("dirty.glb");
    write_distinct_repair_glb(&input);
    let before = std::fs::read(&input).expect("reads input");

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
            "--repair",
            "quat-norm,quat-flip,quat-norm",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);
    assert!(out.contains("would fix[quat-norm]"), "stdout:\n{out}");
    assert!(out.contains("would fix[quat-flip]"), "stdout:\n{out}");
    assert_eq!(
        out.matches("would fix[quat-norm]").count(),
        1,
        "non-adjacent duplicate repairs should be reported once:\n{out}"
    );
    assert_eq!(before, std::fs::read(&input).expect("reads input"));
    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatNorm)
            .expect("inspects dirty input")
            .total_fixed(),
        1
    );
    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatFlip)
            .expect("inspects dirty input")
            .total_fixed(),
        2
    );
}

#[test]
fn fix_dry_run_labels_each_repair_with_its_action() {
    // The distinct-repair fixture needs both a quat-norm (non-unit key)
    // and a quat-flip (hemisphere) repair on the same bone, so the report
    // prints one per-track line per repair. Each line must carry its own
    // action suffix; a swapped or stale Repair::action() would pair the
    // wrong verb with the id.
    let dir = unique_temp_dir("fix-action-labels");
    let input = dir.path().join("dirty.glb");
    write_distinct_repair_glb(&input);

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
            "--repair",
            "quat-norm,quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    let out = stdout(&output);
    let norm_line = out
        .lines()
        .find(|l| l.contains("would fix[quat-norm]"))
        .unwrap_or_else(|| panic!("no quat-norm track line:\n{out}"));
    assert!(
        norm_line.contains("unit-normalized"),
        "quat-norm line must report unit-normalized: {norm_line}"
    );
    let flip_line = out
        .lines()
        .find(|l| l.contains("would fix[quat-flip]"))
        .unwrap_or_else(|| panic!("no quat-flip track line:\n{out}"));
    assert!(
        flip_line.contains("hemisphere-normalized"),
        "quat-flip line must report hemisphere-normalized: {flip_line}"
    );
}

#[test]
fn fix_dry_run_on_clean_input_exits_zero() {
    let dir = unique_temp_dir("fix-dry-run-clean");
    let input = dir.path().join("clean.glb");
    write_clean_glb(&input);

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("0 key(s) would be fixed"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn fix_dry_run_skipped_tracks_do_not_fail_the_check() {
    // A .gltf written by the writer embeds its buffer as a data URI,
    // which fix cannot patch: the track is reported as skipped. The
    // dry-run exit code reflects repairs fix would PERFORM — skipped
    // tracks print loudly but exit 0; detection-only gating is lint's
    // job (the quat-flip check).
    let dir = unique_temp_dir("fix-dry-run-skip");
    let input = dir.path().join("dirty.gltf");
    animsmith_gltf::write::write(&sway_doc(true), &input).expect("writes gltf fixture");

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("skipped[quat-flip]"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn fix_dry_run_conflicts_with_write_targets() {
    for write_flag in [&["-o", "out.glb"][..], &["--in-place"][..]] {
        let output = animsmith()
            .args(["fix", "clip.glb", "--dry-run"])
            .args(write_flag)
            .output()
            .expect("runs animsmith");

        assert_eq!(
            output.status.code(),
            Some(2),
            "--dry-run with {write_flag:?} must be rejected; stdout:\n{}",
            stdout(&output)
        );
        assert!(
            stderr(&output).contains("--dry-run"),
            "stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn fix_default_repairs_write_output() {
    let dir = unique_temp_dir("fix-output");
    let input = dir.path().join("dirty.glb");
    let output_path = dir.path().join("fixed.glb");
    write_flipped_glb(&input);

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--output",
            output_path.to_str().expect("utf-8 output path"),
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(output_path.exists());

    // Analytic oracle: hemisphere normalization must restore exactly
    // the un-flipped source sequence (negation is a lossless bit flip).
    let fixed = animsmith_gltf::load(&output_path).expect("loads fixed output");
    let TrackValues::Quats(quats) = &fixed.clips[0].tracks[0].values else {
        panic!("rotation track expected");
    };
    let expected = sway_quats(false);
    for (got, want) in quats.iter().zip(&expected) {
        assert_eq!(got.to_array(), want.to_array());
    }
}

#[test]
fn fix_write_composes_distinct_repairs() {
    let dir = unique_temp_dir("fix-output-compose");
    let input = dir.path().join("dirty.glb");
    let output_path = dir.path().join("fixed.glb");
    write_distinct_repair_glb(&input);

    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatNorm)
            .expect("inspects dirty input")
            .total_fixed(),
        1
    );
    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatFlip)
            .expect("inspects dirty input")
            .total_fixed(),
        2
    );

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--output",
            output_path.to_str().expect("utf-8 output path"),
            "--repair",
            "quat-norm,quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);
    assert!(out.contains("fixed[quat-norm]"), "stdout:\n{out}");
    assert!(out.contains("fixed[quat-flip]"), "stdout:\n{out}");

    assert_eq!(
        FixSession::inspect(&output_path, GltfRepair::QuatNorm)
            .expect("inspects fixed output")
            .total_fixed(),
        0
    );
    assert_eq!(
        FixSession::inspect(&output_path, GltfRepair::QuatFlip)
            .expect("inspects fixed output")
            .total_fixed(),
        0
    );

    let fixed = animsmith_gltf::load(&output_path).expect("loads fixed output");
    let TrackValues::Quats(quats) = &fixed.clips[0].tracks[0].values else {
        panic!("rotation track expected");
    };
    for (got, want) in quats.iter().zip(sway_quats(false)) {
        assert!(
            got.dot(want).abs() > 1.0 - 1e-5,
            "composed repairs must preserve the represented rotation"
        );
    }
}

#[test]
fn fix_write_dedupes_duplicate_repairs() {
    let dir = unique_temp_dir("fix-output-dedup");
    let input = dir.path().join("dirty.glb");
    let output_path = dir.path().join("fixed.glb");
    write_flipped_glb(&input);

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--output",
            output_path.to_str().expect("utf-8 output path"),
            "--repair",
            "quat-flip,quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);
    assert_eq!(
        out.matches("key(s) fixed across").count(),
        1,
        "duplicate repairs should be reported once:\n{out}"
    );
    assert_eq!(
        FixSession::inspect(&output_path, GltfRepair::QuatFlip)
            .expect("inspects fixed output")
            .total_fixed(),
        0
    );
}

#[test]
fn fix_in_place_writes_selected_repair() {
    let dir = unique_temp_dir("fix-in-place");
    let input = dir.path().join("dirty.glb");
    write_flipped_glb(&input);
    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatFlip)
            .expect("inspects dirty input")
            .total_fixed(),
        2
    );

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--in-place",
            "--repair",
            "quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatFlip)
            .expect("inspects fixed input")
            .total_fixed(),
        0
    );
}

#[test]
fn help_matches_compiled_feature_set() {
    let output = animsmith().arg("--help").output().expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("inspect"));
    assert!(out.contains("measure"));
    assert!(out.contains("lint"));
    assert!(out.contains("transform"));
    assert!(out.contains("fix"));
    assert!(out.contains("scale"));
    assert!(out.contains("generate"));
    assert!(out.contains("diff"));

    // One-line summaries come from the doc comments (clap derives
    // `about` from the first line); pin them so description drift is
    // visible.
    assert!(out.contains("Repair safe mechanical glTF/GLB defects"));
    assert!(out.contains("Apply mechanical clip transforms"));
    assert!(out.contains("Compare animation measurements"));
    // `scale` is unconditional: it is the minimal binary's only
    // evidence-emitting producer, so a feature gate on it would silently
    // remove that surface.
    assert!(out.contains("Rewrite declared linear scale and publish versioned evidence"));
    assert!(out.contains("Generate bounded, versioned pipeline contracts"));

    assert_eq!(out.contains("\n  convert "), cfg!(feature = "fbx"), "{out}");
    assert_eq!(
        out.contains("\n  report "),
        cfg!(feature = "report"),
        "{out}"
    );

    let transform = animsmith()
        .args(["transform", "--help"])
        .output()
        .expect("runs transform help");
    assert!(
        transform.status.success(),
        "stderr:\n{}",
        stderr(&transform)
    );
    assert!(
        stdout(&transform).contains("--prune-constant-tracks"),
        "{}",
        stdout(&transform)
    );

    let diff = animsmith()
        .args(["diff", "--help"])
        .output()
        .expect("runs diff help");
    assert!(diff.status.success(), "stderr:\n{}", stderr(&diff));
    let out = stdout(&diff);
    assert!(out.contains("output-v17"), "{out}");
    assert!(out.contains("output-v16"), "{out}");
    assert!(out.contains("output-v15"), "{out}");
    assert!(out.contains("output-v13"), "{out}");
    assert!(out.contains("measurements-v16"), "{out}");
    assert!(!out.contains("v5"), "{out}");

    let generate = animsmith()
        .args(["generate", "addressability", "--help"])
        .output()
        .expect("runs addressability help");
    assert!(generate.status.success(), "stderr:\n{}", stderr(&generate));
    let out = stdout(&generate);
    assert!(out.contains("<INPUT>"), "{out}");
    assert!(out.contains("[default: json]"), "{out}");
    assert!(
        out.contains("[possible values: json, text, markdown]"),
        "{out}"
    );
    assert!(out.contains("does not claim runtime loading"), "{out}");

    let generate = animsmith()
        .args(["generate", "import-advice", "--help"])
        .output()
        .expect("runs import-advice help");
    assert!(generate.status.success(), "stderr:\n{}", stderr(&generate));
    let out = stdout(&generate);
    assert!(out.contains("<INPUT>"), "{out}");
    assert!(out.contains("[default: json]"), "{out}");
    assert!(
        out.contains("[possible values: json, text, markdown]"),
        "{out}"
    );
    assert!(out.contains("No frame coordinates"), "{out}");
}

#[test]
fn scale_help_requires_every_factor_and_selector_of_appendix_d7() {
    let root = animsmith()
        .args(["scale", "--help"])
        .output()
        .expect("runs scale help");
    assert!(root.status.success(), "stderr:\n{}", stderr(&root));
    let out = stdout(&root);
    assert!(out.contains("whole-document"), "{out}");
    assert!(out.contains("rest-bind"), "{out}");

    let whole = animsmith()
        .args(["scale", "whole-document", "--help"])
        .output()
        .expect("runs whole-document help");
    assert!(whole.status.success(), "stderr:\n{}", stderr(&whole));
    let out = stdout(&whole);
    // Required, not optional: clap renders an optional argument in square
    // brackets, so `<FACTOR>` bare is what pins "no inferred factor".
    assert!(out.contains("--factor <FACTOR>"), "{out}");
    assert!(out.contains("--evidence <EVIDENCE>"), "{out}");
    assert!(out.contains("--format <FORMAT>"), "{out}");
    assert!(out.contains("[default: text]"), "{out}");
    assert!(out.contains("[possible values: text, json]"), "{out}");
    assert!(out.contains("POSITION morph-target deltas"), "{out}");
    assert!(
        !out.contains("--in-place") && !out.contains("--tolerance"),
        "there is no in-place mode and no per-run tolerance flag: {out}"
    );

    let rest_bind = animsmith()
        .args(["scale", "rest-bind", "--help"])
        .output()
        .expect("runs rest-bind help");
    assert!(
        rest_bind.status.success(),
        "stderr:\n{}",
        stderr(&rest_bind)
    );
    let out = stdout(&rest_bind);
    assert!(
        out.contains("--source-skin-index <SOURCE_SKIN_INDEX>"),
        "{out}"
    );
    assert!(
        out.contains("--source-root-node-index <SOURCE_ROOT_NODE_INDEX>"),
        "{out}"
    );
    assert!(out.contains("--expected-factor <EXPECTED_FACTOR>"), "{out}");
    assert!(out.contains("--evidence <EVIDENCE>"), "{out}");

    // Omitting any required selector is a usage error, not a default.
    let missing = animsmith()
        .args(["scale", "rest-bind", "rig.glb", "-o", "out.glb"])
        .output()
        .expect("runs rest-bind without selectors");
    assert_eq!(missing.status.code(), Some(2), "{}", stderr(&missing));
    for required in [
        "--source-skin-index",
        "--source-root-node-index",
        "--expected-factor",
        "--evidence",
    ] {
        assert!(
            stderr(&missing).contains(required),
            "{required} is not reported as required:\n{}",
            stderr(&missing)
        );
    }
}

#[cfg(feature = "fbx")]
#[test]
fn convert_help_exposes_texture_recipe_static_bake_and_machine_evidence_contract() {
    let output = animsmith()
        .args(["convert", "--help"])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("--material-texture-recipe <PATH>"), "{out}");
    assert!(out.contains("--bake-static-mesh-transforms"), "{out}");
    assert!(out.contains("--format <FORMAT>"), "{out}");
    assert!(out.contains("[default: text]"), "{out}");
    assert!(out.contains("[possible values: text, json]"), "{out}");

    let conflict = animsmith()
        .args([
            "convert",
            "input.glb",
            "-o",
            "output.glb",
            "--animation-only",
            "--bake-static-mesh-transforms",
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(conflict.status.code(), Some(2));
    assert!(
        stderr(&conflict).contains("cannot be used with"),
        "{}",
        stderr(&conflict)
    );

    let recipe_conflict = animsmith()
        .args([
            "convert",
            "input.glb",
            "-o",
            "output.glb",
            "--animation-only",
            "--material-texture-recipe",
            "recipe.toml",
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(recipe_conflict.status.code(), Some(2));
    assert!(
        stderr(&recipe_conflict).contains("cannot be used with"),
        "{}",
        stderr(&recipe_conflict)
    );
}

#[test]
fn fix_help_lists_repair_possible_values() {
    let output = animsmith()
        .args(["fix", "--help"])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.contains("[possible values: quat-norm, quat-flip]"),
        "stdout:\n{out}"
    );
}

#[test]
fn version_uses_the_composed_build_version_at_the_cli_boundary() {
    let output = animsmith()
        .arg("--version")
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stderr(&output).is_empty(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.starts_with(concat!("animsmith ", env!("CARGO_PKG_VERSION"))),
        "{out}"
    );
}

#[test]
fn measure_json_uses_versioned_envelope() {
    let output = animsmith()
        .args([
            "measure",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    assert_eq!(json["schema_version"], 17);
    assert_eq!(json["schema"], CURRENT_OUTPUT_SCHEMA_ID);
    assert_eq!(json["tool"]["name"], "animsmith");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(json["tool"]["source"].is_object());
    let expected_revision = option_env!("ANIMSMITH_GIT_REVISION");
    assert_eq!(
        json["tool"]["source"]["revision"].as_str(),
        expected_revision
    );
    let expected_dirty =
        option_env!("ANIMSMITH_GIT_DIRTY").and_then(|value| value.parse::<bool>().ok());
    assert_eq!(json["tool"]["source"]["dirty"].as_bool(), expected_dirty);
    if let Some(revision) = expected_revision {
        assert_eq!(revision.len(), 40, "full source revision: {revision}");
        assert!(revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    assert_eq!(json["command"], "measure");
    assert_eq!(json["summary"]["files"], 1);

    let files = json["files"].as_array().expect("files array");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["rig"]["profile"], "unknown");
    assert!(
        files[0].get("prediction_provenance").is_none(),
        "measure output must not carry engine prediction provenance"
    );
    assert!(
        files[0].get("checks").is_none(),
        "measure output must not carry lint checks"
    );
    assert_eq!(files[0]["measurements"]["schema_version"], 16);
    assert_eq!(files[0]["measurements"]["schema"], MEASUREMENTS_SCHEMA_ID);
    assert!(files[0]["measurements"]["clips"]["walk"]["duration_s"].is_number());
    assert_eq!(
        files[0]["measurements"]["clips"]["walk"]["bone_channels"],
        json!([
            {
                "bone_index": 0,
                "bone_name": "root",
                "properties": ["translation"]
            },
            {
                "bone_index": 1,
                "bone_name": "hips",
                "properties": ["rotation"]
            }
        ])
    );
    let loop_bones = files[0]["measurements"]["clips"]["walk"]["loop_continuity"]["bones"]
        .as_array()
        .expect("measurable clip exposes per-bone loop continuity");
    assert_eq!(loop_bones.len(), 3);
    for (bone, (index, name)) in loop_bones
        .iter()
        .zip([(0, "root"), (1, "hips"), (2, "foot")])
    {
        assert_eq!(bone["bone_index"], index);
        assert_eq!(bone["bone_name"], name);
        assert!(bone["position_delta_m"].is_number());
        assert!(bone["rotation_delta_deg"].is_number());
        assert!(bone["seam_velocity_delta_mps"].is_number());
        assert!(bone["seam_angular_velocity_delta_degps"].is_number());
    }
}

#[test]
fn inspect_and_measure_expose_case_tolerant_rig_resolution_provenance() {
    let dir = unique_temp_dir("case-tolerant-rig-profile");
    let input = dir.path().join("case-only-humanoid.gltf");
    write_humanoid_profile_gltf(&input, "Humanoid_");
    let named_config = write_config(
        dir.path(),
        "humanoid.toml",
        "[rig]\nprofile = \"humanoid\"\n",
    );

    for config in [None, Some(&named_config)] {
        let mut measure = animsmith();
        if let Some(config) = config {
            measure.args(["--config", config.to_str().expect("UTF-8 config")]);
        }
        let output = measure
            .args([
                "measure",
                input.to_str().expect("UTF-8 input"),
                "--format",
                "json",
            ])
            .output()
            .expect("runs measure");
        assert!(output.status.success(), "{}", stderr(&output));
        let json: Value = serde_json::from_slice(&output.stdout).expect("measure JSON");
        assert_output_schema_valid(&json);
        let rig = &json["files"][0]["rig"];
        assert_eq!(rig["profile"], "humanoid");
        assert_eq!(rig["resolved_roles"]["left_foot"], "Humanoid_ L Foot");
        assert_eq!(
            rig["resolved_role_policies"]["left_foot"],
            "ascii-case-insensitive"
        );
        assert_eq!(rig["resolved_role_policies"]["root"], "exact");
    }

    let inspect = animsmith()
        .args(["inspect", input.to_str().expect("UTF-8 input")])
        .output()
        .expect("runs inspect");
    assert!(inspect.status.success(), "{}", stderr(&inspect));
    assert!(
        stdout(&inspect).contains("left_foot    -> Humanoid_ L Foot (ascii-case-insensitive)"),
        "{}",
        stdout(&inspect)
    );
}

#[test]
fn angular_loop_seam_is_versioned_and_configurable_at_the_cli_boundary() {
    let dir = unique_temp_dir("angular-loop-seam");
    let input = dir.path().join("angular-cusp.glb");
    write_angular_cusp_glb(&input);
    let config = write_config(
        dir.path(),
        "angular-loop.toml",
        "[clips.angular_cusp]\nloop = true\n",
    );

    let baseline = animsmith()
        .arg("--config")
        .arg(&config)
        .arg("lint")
        .arg(&input)
        .args(["--select", "loop-seam-rot", "--format", "json"])
        .output()
        .expect("runs angular seam lint");
    assert_eq!(
        baseline.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&baseline)
    );
    let baseline: Value = serde_json::from_slice(&baseline.stdout).expect("valid lint JSON");
    assert_output_schema_valid(&baseline);
    assert_eq!(baseline["files"][0]["measurements"]["schema_version"], 16);
    assert_eq!(
        baseline["files"][0]["measurements"]["schema"],
        MEASUREMENTS_SCHEMA_ID
    );
    let mut missing_angular_evidence = baseline.clone();
    missing_angular_evidence["files"][0]["measurements"]["clips"]["angular_cusp"]
        ["loop_continuity"]["bones"][0]
        .as_object_mut()
        .expect("bone measurement object")
        .remove("seam_angular_velocity_delta_degps");
    assert!(
        !output_validator().is_valid(&missing_angular_evidence),
        "measurements-v16 requires angular seam evidence in every loop-continuity row"
    );
    let bones =
        baseline["files"][0]["measurements"]["clips"]["angular_cusp"]["loop_continuity"]["bones"]
            .as_array()
            .expect("per-bone loop continuity");
    assert_eq!(bones.len(), 2);
    for (bone, (index, name)) in bones.iter().zip([(0, "root"), (1, "spine")]) {
        assert_eq!(bone["bone_index"], index);
        assert_eq!(bone["bone_name"], name);
        assert_eq!(bone["seam_velocity_delta_mps"], 0.0);
    }
    let spine = &bones[1];
    assert!(
        spine["seam_angular_velocity_delta_degps"]
            .as_f64()
            .expect("angular seam metric")
            > 100.0,
        "expected analytic angular cusp: {spine:#}"
    );
    let check = baseline["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "loop-seam-rot")
        .expect("angular seam check");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "complete");
    assert_eq!(check["evaluated_scopes"][0]["code"], "loop_seam_rotation");
    assert_eq!(check["findings"][0]["severity"], "error");
    assert_eq!(check["findings"][0]["bone"], "spine");
    assert_eq!(
        check["findings"][0]["measured"],
        spine["seam_angular_velocity_delta_degps"]
    );
    assert_eq!(check["findings"][0]["expected"], 5.0);

    let relaxed = write_config(
        dir.path(),
        "relaxed-angular-loop.toml",
        "[clips.angular_cusp]\nloop = true\n\n[checks.loop-seam-rot]\nmax_angular_velocity_delta_degps = 200.0\n",
    );
    let relaxed = animsmith()
        .arg("--config")
        .arg(&relaxed)
        .arg("lint")
        .arg(&input)
        .args(["--select", "loop-seam-rot", "--format", "json"])
        .output()
        .expect("runs configured angular seam lint");
    assert_eq!(
        relaxed.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&relaxed)
    );
    let relaxed: Value = serde_json::from_slice(&relaxed.stdout).expect("valid lint JSON");
    let check = relaxed["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "loop-seam-rot")
        .expect("angular seam check");
    assert_eq!(check["findings"], json!([]));
    assert_eq!(check["evaluation"], "complete");
}

#[test]
fn measure_text_escapes_controls_in_clip_and_mesh_names() {
    let dir = unique_temp_dir("measure-text-controls");
    let input = dir.path().join("hostile.glb");
    write_hostile_glb(&input, HOSTILE_PRESENTATION_TEXT, false);

    let output = animsmith()
        .arg("measure")
        .arg(&input)
        .args(["--format", "text"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let text = stdout(&output);
    assert_hostile_text_is_escaped(&text);
    assert!(text.contains("Δω=0.00°/s"), "angular aggregate: {text}");
    assert_eq!(
        text.matches("\\n").count(),
        5,
        "clip, mesh, node instance, and two source nodes: {text}"
    );
    assert_eq!(
        text.matches("\\u{1b}").count(),
        5,
        "clip, mesh, node instance, and two source nodes: {text}"
    );
    assert_eq!(
        text.matches("\\u{2028}").count(),
        5,
        "clip, mesh, node instance, and two source nodes: {text}"
    );
    assert_eq!(
        text.matches("\\u{202e}").count(),
        5,
        "clip, mesh, node instance, and two source nodes: {text}"
    );
}

#[test]
fn inspect_fix_and_transform_escape_asset_derived_text() {
    let dir = unique_temp_dir("command-text-controls");
    let clean = dir.path().join("hostile-clean.glb");
    let flipped = dir.path().join("hostile-flipped.glb");
    let transformed = dir.path().join("transformed.glb");
    write_hostile_glb(&clean, HOSTILE_PRESENTATION_TEXT, false);
    write_hostile_glb(&flipped, HOSTILE_PRESENTATION_TEXT, true);

    let inspect = animsmith()
        .arg("inspect")
        .arg(&clean)
        .output()
        .expect("runs inspect");
    assert_eq!(inspect.status.code(), Some(0), "{}", stderr(&inspect));
    assert_hostile_text_is_escaped(&stdout(&inspect));

    let transform = animsmith()
        .arg("transform")
        .arg(&clean)
        .args(["--hold-extend", "0.25", "--output"])
        .arg(&transformed)
        .output()
        .expect("runs transform");
    assert_eq!(transform.status.code(), Some(0), "{}", stderr(&transform));
    assert_hostile_text_is_escaped(&stdout(&transform));

    let fix = animsmith()
        .arg("fix")
        .arg(&flipped)
        .args(["--dry-run", "--repair", "quat-flip"])
        .output()
        .expect("runs fix");
    assert_eq!(fix.status.code(), Some(1), "{}", stderr(&fix));
    assert_hostile_text_is_escaped(&stdout(&fix));
}

#[cfg(unix)]
#[test]
fn measure_text_escapes_controls_in_the_input_path() {
    let dir = unique_temp_dir("measure-text-path-controls");
    let hostile_name = "asset\nforged\u{1b}[31m.gltf";
    let input = dir.path().join(hostile_name);
    std::fs::copy(fixture("rig.gltf"), &input).expect("copies self-contained glTF fixture");

    let output = animsmith()
        .arg("measure")
        .arg(&input)
        .args(["--format", "text"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let text = stdout(&output);
    assert!(
        !text.contains(hostile_name),
        "raw path controls leaked:\n{text}"
    );
    assert!(text.contains("asset\\nforged\\u{1b}[31m.gltf"), "{text}");
}

#[cfg(unix)]
#[test]
fn measure_text_renderer_escapes_hostile_asset_text_and_input_path() {
    let dir = unique_temp_dir("measure-renderer-controls");
    let hostile_path_text = "path\nasset\u{1b}[32m\u{2028}\u{2029}\u{2066}";
    let input = dir.path().join(format!("asset-{hostile_path_text}.glb"));
    write_hostile_glb(&input, HOSTILE_PRESENTATION_TEXT, false);

    let output = animsmith()
        .arg("measure")
        .arg(&input)
        .args(["--format", "text"])
        .output()
        .expect("runs measure");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));

    let text = stdout(&output);
    assert_hostile_text_is_escaped(&text);
    assert!(
        !text.contains(hostile_path_text),
        "raw path leaked:\n{text}"
    );
    let escaped_path = "path\\nasset\\u{1b}[32m\\u{2028}\\u{2029}\\u{2066}";
    let escaped_asset = "forged\\nline\\u{1b}[31m\\u{2028}\\u{2029}\\u{202e}";
    assert_eq!(
        text.matches(escaped_path).count(),
        1,
        "input path should be escaped once:\n{text}"
    );
    assert_eq!(
        text.matches(escaped_asset).count(),
        5,
        "clip, mesh-definition, node-instance, and source-node names should each be escaped:\n{text}"
    );
    assert!(
        text.lines()
            .next()
            .is_some_and(|line| line.contains(escaped_path)),
        "escaped path should remain the heading:\n{text}"
    );
}

#[cfg(unix)]
#[test]
fn command_text_escapes_output_and_operator_error_paths() {
    let dir = unique_temp_dir("command-path-controls");
    let input = dir.path().join("clean.glb");
    let flipped = dir.path().join("flipped.glb");
    write_clean_glb(&input);
    write_flipped_glb(&flipped);
    let hostile_output = dir.path().join(format!("{HOSTILE_PRESENTATION_TEXT}.glb"));

    let transform = animsmith()
        .arg("transform")
        .arg(&input)
        .args(["--hold-extend", "0.25", "--output"])
        .arg(&hostile_output)
        .output()
        .expect("runs transform");
    assert_eq!(transform.status.code(), Some(0), "{}", stderr(&transform));
    assert_hostile_text_is_escaped(&stdout(&transform));

    let hostile_fix_output = dir
        .path()
        .join(format!("fixed-{HOSTILE_PRESENTATION_TEXT}.glb"));
    let fix = animsmith()
        .arg("fix")
        .arg(&flipped)
        .args(["--repair", "quat-flip", "--output"])
        .arg(&hostile_fix_output)
        .output()
        .expect("runs fix");
    assert_eq!(fix.status.code(), Some(0), "{}", stderr(&fix));
    assert_hostile_text_is_escaped(&stdout(&fix));

    let missing = dir
        .path()
        .join(format!("missing-{HOSTILE_PRESENTATION_TEXT}.glb"));
    let inspect = animsmith()
        .arg("inspect")
        .arg(&missing)
        .output()
        .expect("runs inspect");
    assert_eq!(inspect.status.code(), Some(2));
    assert_hostile_text_is_escaped(&stderr(&inspect));
}

#[cfg(all(feature = "report", unix))]
#[test]
fn report_text_escapes_its_output_path() {
    let dir = unique_temp_dir("report-path-controls");
    let output_path = dir
        .path()
        .join(format!("report-{HOSTILE_PRESENTATION_TEXT}.html"));
    let output = animsmith()
        .arg("report")
        .arg(fixture("rig.gltf"))
        .arg("--output")
        .arg(&output_path)
        .output()
        .expect("runs report");

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_hostile_text_is_escaped(&stdout(&output));
    assert!(output_path.is_file(), "report output was not written");
}

#[test]
fn embedded_contract_types_emit_the_published_v16_envelope() {
    let doc = Document::default();
    let config = animsmith_core::Config::default();
    let roles = animsmith_core::ResolvedRoles::default();
    let grids = animsmith_core::MetricGrids::new(&doc);
    let ctx = animsmith_core::CheckCtx::new(&grids, &roles, &config);
    let checks = animsmith_core::evaluate_checks(
        &ctx,
        &animsmith_core::all_checks(),
        animsmith_core::CheckSelection::All,
    )
    .expect("built-in catalog evaluates");
    let file = animsmith_core::LintFileReportV16::new(
        "embedded.glb",
        embedded_input_identity(),
        animsmith_core::RigInfo::from_resolved(&doc, &roles)
            .expect("roles were resolved from this document"),
        None,
        checks,
        animsmith_core::MeasurementContract::new(
            animsmith_core::measure::measure_document(&grids, &roles, &config),
            animsmith_core::measure::measure_assets(&doc),
        )
        .expect("measured evidence is finite"),
    )
    .expect("bounded lint file");
    let envelope = animsmith_core::LintEnvelopeV16::new(
        animsmith_core::ToolInfo::animsmith(animsmith_core::ToolSource::new(None, None)),
        vec![file],
    )
    .expect("bounded lint envelope");

    let json = serde_json::to_value(envelope).expect("embedded envelope serializes");
    assert_output_v16_schema_valid(&json);
    assert_eq!(json["schema"], OUTPUT_V16_SCHEMA_ID);
    assert_eq!(
        json["files"][0]["measurements"]["schema"],
        animsmith_core::MEASUREMENTS_SCHEMA_ID
    );
}

#[test]
fn published_v17_schema_requires_matching_role_policy_provenance() {
    let output = animsmith()
        .args([
            "measure",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let base: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&base);
    let validator = output_validator();

    let mut missing_policy = base.clone();
    missing_policy["files"][0]["rig"]["resolved_roles"] = json!({ "hips": "hips" });
    missing_policy["files"][0]["rig"]["resolved_role_policies"] = json!({});

    let mut orphan_policy = base.clone();
    orphan_policy["files"][0]["rig"]["resolved_roles"] = json!({});
    orphan_policy["files"][0]["rig"]["resolved_role_policies"] = json!({ "hips": "exact" });

    let mut unknown_role = base.clone();
    unknown_role["files"][0]["rig"]["resolved_roles"] = json!({ "tail": "tail" });
    unknown_role["files"][0]["rig"]["resolved_role_policies"] = json!({ "tail": "exact" });

    let mut ambiguous_with_roles = base;
    ambiguous_with_roles["files"][0]["rig"]["resolution_outcome"] = json!("ambiguous_folded_match");
    ambiguous_with_roles["files"][0]["rig"]["resolved_roles"] = json!({ "hips": "hips" });
    ambiguous_with_roles["files"][0]["rig"]["resolved_role_policies"] = json!({ "hips": "exact" });

    for (name, invalid) in [
        ("missing policy", missing_policy),
        ("orphan policy", orphan_policy),
        ("unknown role", unknown_role),
        ("ambiguous outcome with roles", ambiguous_with_roles),
    ] {
        assert!(
            !validator.is_valid(&invalid),
            "output-v16 accepted {name}: {invalid:#}"
        );
    }
}

#[test]
fn published_v16_schema_accepts_and_distinguishes_every_prediction_facet_lifecycle() {
    let dir = unique_temp_dir("prediction-schema-lifecycle");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let source = animsmith_gltf::load_source(&input).expect("source loads with raw facts");
    let clip_names = source
        .document()
        .clips
        .iter()
        .map(|clip| clip.name.clone())
        .collect::<Vec<_>>();
    let resolved = animsmith_engine::resolve_static(animsmith_engine::EngineDeclaration {
        selection: Some(animsmith_engine::ProfileSelection::new(
            "bevy",
            1,
            "0.19.0",
            "gltf-asset-loader",
        )),
        ..Default::default()
    })
    .expect("profile declaration is valid")
    .expect("profile selected")
    .resolve_input_v2_iter(
        source.source_facts().format(),
        clip_names.iter().map(String::as_str),
    )
    .expect("fixture format is accepted");
    let provenance = animsmith_engine::project_prediction_provenance_v3(&resolved, &source)
        .expect("same-load provenance projects");

    let prediction_check = |check_id: &'static str, available: bool, unavailable: bool| {
        let available_scope = animsmith_core::EvaluationScope::new(
            animsmith_core::EvaluationScopeCode::custom("test:available"),
        );
        let unavailable_scope = animsmith_core::EvaluationScope::new(
            animsmith_core::EvaluationScopeCode::custom("test:unavailable"),
        );
        let mut facets = Vec::new();
        let mut evaluated = Vec::new();
        let mut findings = Vec::new();
        if available {
            facets.push(
                animsmith_core::EnginePredictionFacetV3::available(
                    available_scope.clone(),
                    animsmith_core::EnginePredictionBasisV2::new(vec![
                        animsmith_core::PredictionBasisReferenceV2::v1(
                            animsmith_core::PredictionBasisReferenceV1::profile_fact(
                                "accepted_inputs",
                            )
                            .expect("known profile fact"),
                        ),
                    ])
                    .expect("nonempty basis"),
                )
                .expect("available facet"),
            );
            evaluated.push(available_scope.clone());
            findings.push(
                animsmith_core::Finding::new(
                    check_id,
                    animsmith_core::Severity::Note,
                    "available facet finding",
                )
                .prediction_scope(available_scope),
            );
        }
        if unavailable {
            facets.push(
                animsmith_core::EnginePredictionFacetV3::required_unavailable(
                    unavailable_scope,
                    animsmith_core::EnginePredictionBasisV2::new(Vec::new())
                        .expect("empty unavailable basis prefix"),
                    vec![animsmith_core::PredictionUnavailableReasonV2::ProjectIntentUnavailable],
                )
                .expect("required-unavailable facet"),
            );
        }
        let prediction =
            animsmith_core::EnginePredictionV3::new(provenance.identity().clone(), facets)
                .expect("canonical prediction");
        animsmith_core::CheckEvaluation::evaluated(
            check_id,
            animsmith_core::CheckOutput::from_coverage(findings, evaluated, Vec::new())
                .with_engine_prediction_v3(prediction),
        )
        .expect("prediction lifecycle is valid")
    };
    let measurement_scope = animsmith_core::EvaluationScope::new(
        animsmith_core::EvaluationScopeCode::custom("test:measurement"),
    );
    let measurement_prediction = animsmith_core::EnginePredictionV3::new(
        provenance.identity().clone(),
        vec![
            animsmith_core::EnginePredictionFacetV3::available(
                measurement_scope.clone(),
                animsmith_core::EnginePredictionBasisV2::new(vec![
                    animsmith_core::PredictionBasisReferenceV2::v1(
                        animsmith_core::PredictionBasisReferenceV1::measurement_v16(
                            animsmith_core::MeasurementPointerV1::new(
                                "/measurements/material_resource_coverage",
                            )
                            .expect("canonical measurement pointer"),
                            animsmith_core::PredictionScalarV1::token("unavailable")
                                .expect("bounded measurement token"),
                        ),
                    ),
                ])
                .expect("nonempty measurement basis"),
            )
            .expect("available measurement facet"),
        ],
    )
    .expect("canonical measurement prediction");
    let measurement_check = animsmith_core::CheckEvaluation::evaluated(
        "test:measurement",
        animsmith_core::CheckOutput::from_coverage(Vec::new(), vec![measurement_scope], Vec::new())
            .with_engine_prediction_v3(measurement_prediction),
    )
    .expect("measurement prediction lifecycle is valid");
    let checks = vec![
        prediction_check("test:available-only", true, false),
        prediction_check("test:mixed", true, true),
        prediction_check("test:unavailable-only", false, true),
        measurement_check,
    ];
    let rig = animsmith_core::RigInfo::from_resolved(
        source.document(),
        &animsmith_core::ResolvedRoles::default(),
    )
    .expect("empty roles match the source document");
    let wrong_scope = animsmith_core::EvaluationScope::new(
        animsmith_core::EvaluationScopeCode::custom("test:measurement"),
    );
    let wrong_prediction = animsmith_core::EnginePredictionV3::new(
        provenance.identity().clone(),
        vec![
            animsmith_core::EnginePredictionFacetV3::available(
                wrong_scope.clone(),
                animsmith_core::EnginePredictionBasisV2::new(vec![
                    animsmith_core::PredictionBasisReferenceV2::v1(
                        animsmith_core::PredictionBasisReferenceV1::measurement_v16(
                            animsmith_core::MeasurementPointerV1::new(
                                "/measurements/schema_version",
                            )
                            .expect("canonical measurement pointer"),
                            animsmith_core::PredictionScalarV1::UnsignedInteger { value: 14 },
                        ),
                    ),
                ])
                .expect("nonempty measurement basis"),
            )
            .expect("available measurement facet"),
        ],
    )
    .expect("canonical wrong-value prediction");
    let wrong_check = animsmith_core::CheckEvaluation::evaluated(
        "test:measurement",
        animsmith_core::CheckOutput::from_coverage(Vec::new(), vec![wrong_scope], Vec::new())
            .with_engine_prediction_v3(wrong_prediction),
    )
    .expect("prediction lifecycle is valid before file measurement binding");
    let wrong = animsmith_core::LintFileReportV16::new(
        input.display().to_string(),
        source.source_facts().primary_identity().clone(),
        rig.clone(),
        Some(provenance.clone()),
        vec![wrong_check],
        animsmith_core::MeasurementContract::new(
            BTreeMap::new(),
            animsmith_core::measure::AssetMeasurements::default(),
        )
        .expect("empty measurements are valid"),
    );
    assert!(matches!(
        wrong,
        Err(animsmith_core::OutputContractError::InvalidPrediction(
            animsmith_core::PredictionContractError::MeasurementValueMismatch(_)
        ))
    ));
    let file = animsmith_core::LintFileReportV16::new(
        input.display().to_string(),
        source.source_facts().primary_identity().clone(),
        rig,
        Some(provenance),
        checks,
        animsmith_core::MeasurementContract::new(
            BTreeMap::new(),
            animsmith_core::measure::AssetMeasurements::default(),
        )
        .expect("empty measurements are valid"),
    )
    .expect("bounded prediction report");
    let envelope = animsmith_core::LintEnvelopeV16::new(
        animsmith_core::ToolInfo::animsmith(animsmith_core::ToolSource::new(None, None)),
        vec![file],
    )
    .expect("bounded prediction envelope");
    let valid = serde_json::to_value(envelope).expect("prediction envelope serializes");
    assert_output_v16_schema_valid(&valid);
    assert_eq!(
        valid["summary"]["prediction_facets"],
        json!({
            "available": 3,
            "required_prediction_unavailable": 2
        })
    );
    assert_eq!(valid["files"][0]["checks"][0]["evaluation"], "complete");
    assert_eq!(valid["files"][0]["checks"][1]["evaluation"], "partial");
    assert_eq!(
        valid["files"][0]["checks"][2]["evaluation"],
        "not_evaluated"
    );

    let mut unknown = valid.clone();
    unknown["files"][0]["checks"][0]["prediction"]["unexpected"] = json!(true);
    assert!(!output_v16_validator().is_valid(&unknown));

    let mut pointer_at_limit = valid.clone();
    pointer_at_limit["files"][0]["checks"][3]["prediction"]["facets"][0]["basis"]["references"]
        [0]["reference"]["pointer"] = json!(format!("/measurements{}", "/x".repeat(127)));
    assert!(output_v16_validator().is_valid(&pointer_at_limit));
    let mut pointer_above_limit = pointer_at_limit;
    pointer_above_limit["files"][0]["checks"][3]["prediction"]["facets"][0]["basis"]["references"]
        [0]["reference"]["pointer"] = json!(format!("/measurements{}", "/x".repeat(128)));
    assert!(!output_v16_validator().is_valid(&pointer_above_limit));

    let report_path = dir.path().join("prediction-pointer.json");
    std::fs::write(
        &report_path,
        serde_json::to_vec(&valid).expect("serializes valid prediction report"),
    )
    .expect("writes valid prediction report");
    let output = animsmith()
        .args(["diff"])
        .arg(&report_path)
        .arg(&report_path)
        .output()
        .expect("reads valid measurement-backed prediction report");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));

    let mut wrong_measurement_value = valid.clone();
    wrong_measurement_value["files"][0]["measurements"]["material_resource_coverage"] =
        json!("complete");
    std::fs::write(
        &report_path,
        serde_json::to_vec(&wrong_measurement_value)
            .expect("serializes measurement-pointer mismatch"),
    )
    .expect("writes measurement-pointer mismatch");
    let output = animsmith()
        .args(["diff"])
        .arg(&report_path)
        .arg(&report_path)
        .output()
        .expect("rejects measurement-pointer mismatch");
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let error = stderr(&output);
    assert!(
        error.contains("measurement pointer \"/measurements/material_resource_coverage\""),
        "{error}"
    );
    assert!(
        error.contains("disagrees with measurements contract"),
        "{error}"
    );

    let mut invalid_reason = valid;
    invalid_reason["files"][0]["checks"][1]["prediction"]["facets"][1]["reasons"][0] =
        json!("not a valid reason");
    assert!(!output_v16_validator().is_valid(&invalid_reason));
}

#[test]
fn output_schema_rejects_every_empty_custom_check_identifier() {
    let check = animsmith_core::CheckEvaluation::evaluated(
        "custom",
        animsmith_core::CheckOutput::from_coverage(
            Vec::new(),
            vec![animsmith_core::EvaluationScope::new(
                animsmith_core::EvaluationScopeCode::custom("test:complete"),
            )],
            vec![
                animsmith_core::CoverageGap::new(
                    animsmith_core::CoverageGapCode::custom("test:gap"),
                    "missing evidence",
                )
                .scope(animsmith_core::EvaluationScope::new(
                    animsmith_core::EvaluationScopeCode::custom("test:missing"),
                )),
            ],
        ),
    )
    .expect("nonempty custom identifiers are valid");
    let doc = Document::default();
    let roles = animsmith_core::ResolvedRoles::default();
    let envelope = animsmith_core::LintEnvelopeV16::new(
        animsmith_core::ToolInfo::animsmith(animsmith_core::ToolSource::new(None, None)),
        vec![
            animsmith_core::LintFileReportV16::new(
                "embedded.glb",
                embedded_input_identity(),
                animsmith_core::RigInfo::from_resolved(&doc, &roles)
                    .expect("empty roles match an empty document"),
                None,
                vec![check],
                animsmith_core::MeasurementContract::new(
                    BTreeMap::new(),
                    animsmith_core::measure::AssetMeasurements::default(),
                )
                .expect("empty measurements are valid"),
            )
            .expect("bounded lint file"),
        ],
    )
    .expect("bounded lint envelope");
    let valid = serde_json::to_value(envelope).expect("embedded envelope serializes");
    assert_output_v16_schema_valid(&valid);

    for pointer in [
        "/files/0/checks/0/check_id",
        "/files/0/checks/0/evaluated_scopes/0/code",
        "/files/0/checks/0/gaps/0/code",
        "/files/0/checks/0/gaps/0/scope/code",
    ] {
        let mut invalid = valid.clone();
        *invalid.pointer_mut(pointer).expect("fixture path exists") = json!("");
        assert!(
            !output_v16_validator().is_valid(&invalid),
            "schema accepted an empty identifier at {pointer}"
        );
    }
}

#[test]
fn lint_json_uses_versioned_envelope() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["schema_version"], 17);
    assert_eq!(json["schema"], CURRENT_OUTPUT_SCHEMA_ID);
    assert_eq!(json["tool"]["name"], "animsmith");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["command"], "lint");
    assert_eq!(json["summary"]["files"], 1);
    assert_eq!(
        json["summary"]["prediction_facets"],
        json!({
            "available": 0,
            "required_prediction_unavailable": 0
        })
    );
    assert!(
        json["files"][0].get("prediction_provenance").is_some(),
        "lint provenance is a required nullable field"
    );
    assert!(json["files"][0]["prediction_provenance"].is_null());
    assert!(json["files"][0]["checks"].is_array());
    assert_eq!(json["files"][0]["measurements"]["schema_version"], 16);
    assert_eq!(
        json["files"][0]["measurements"]["schema"],
        MEASUREMENTS_SCHEMA_ID
    );
    assert!(json["files"][0]["measurements"]["clips"]["walk"]["duration_s"].is_number());
    let actual_ids: BTreeSet<_> = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .map(|check| check["check_id"].as_str().expect("check id"))
        .collect();
    assert_eq!(actual_ids, EXPECTED_CHECK_IDS.into_iter().collect());
    let constant_nonunit = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "constant-nonunit-scale")
        .expect("opt-in scale-channel record");
    assert_eq!(constant_nonunit["selection"], "selected");
    assert_eq!(constant_nonunit["configuration"], "disabled");
    assert_eq!(constant_nonunit["evaluation"], "not_evaluated");
    assert_eq!(
        constant_nonunit
            .as_object()
            .expect("check object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        [
            "check_id",
            "selection",
            "configuration",
            "applicability",
            "evaluation",
            "findings",
        ]
        .into_iter()
        .collect()
    );
    let engine_addressability = lint_check(&json, "engine-addressability");
    assert_eq!(engine_addressability["selection"], "selected");
    assert_eq!(engine_addressability["configuration"], "enabled");
    assert_eq!(engine_addressability["applicability"], "not_applicable");
    assert_eq!(engine_addressability["evaluation"], "not_evaluated");
    assert!(engine_addressability.get("prediction").is_none());
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
}

#[test]
fn measure_and_lint_json_retain_each_primary_file_identity_in_argument_order() {
    let dir = unique_temp_dir("input-identities");
    let first = dir.path().join("first.glb");
    let second = dir.path().join("second.glb");
    write_clean_glb(&first);
    write_flipped_glb(&second);
    let expected = [input_identity_json(&first), input_identity_json(&second)];

    for command in ["measure", "lint"] {
        let output = animsmith()
            .args([
                command,
                first.to_str().expect("utf-8 fixture path"),
                second.to_str().expect("utf-8 fixture path"),
                "--format",
                "json",
            ])
            .output()
            .expect("runs JSON command");
        assert!(
            matches!(output.status.code(), Some(0 | 1)),
            "{command}: {}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        let files = json["files"].as_array().expect("file records");
        assert_eq!(files.len(), 2);
        for (index, expected_input) in expected.iter().enumerate() {
            assert_eq!(
                files[index]["path"],
                [first.as_path(), second.as_path()][index]
                    .display()
                    .to_string()
            );
            assert_eq!(
                files[index]["input"], *expected_input,
                "{command} row {index}"
            );
            assert_eq!(
                files[index]["input"]["sha256"].as_str().map(str::len),
                Some(64)
            );
            assert_eq!(files[index]["input"]["bytes"], expected_input["bytes"]);
        }
        assert_ne!(files[0]["input"], files[1]["input"]);
    }
}

#[test]
fn cli_and_embedded_role_resolution_are_identical() {
    let dir = unique_temp_dir("resolver-parity");
    let input = dir.path().join("trajectory.glb");
    write_root_trajectory_glb(&input);
    let config_path = write_config(
        dir.path(),
        "roles.toml",
        "[rig]\nprofile = \"ue-mannequin\"\n[rig.roles]\nhips = \"spine\"\n",
    );
    let config: animsmith_core::Config = serde_json::from_value(json!({
        "rig": {
            "profile": "ue-mannequin",
            "roles": { "hips": "spine" }
        }
    }))
    .expect("embedded config");
    let doc = animsmith_gltf::load(&input).expect("loads fixture for embedding");
    let embedded = animsmith_core::resolve_configured_roles(&doc.skeleton, &config.rig);
    let embedded_roles: BTreeMap<_, _> = embedded
        .iter()
        .map(|(role, bone)| (role.as_str(), doc.skeleton.bones[bone].name.as_str()))
        .collect();

    let output = animsmith()
        .arg("--config")
        .arg(&config_path)
        .args(["lint", input.to_str().unwrap(), "--format", "json"])
        .output()
        .expect("runs animsmith");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    assert_eq!(json["files"][0]["rig"]["profile"], embedded.profile);
    assert_eq!(
        json["files"][0]["rig"]["resolved_roles"],
        json!(embedded_roles)
    );
    assert_eq!(
        json["files"][0]["rig"]["resolved_role_policies"]["hips"],
        "explicit"
    );
    let trajectory = &json["files"][0]["measurements"]["clips"]["trajectory"]["root_trajectory"];
    assert_eq!(
        trajectory,
        &json!({
            "bone_index": 0,
            "bone_name": "root",
            "source_role": "root",
            "translation": {
                "horizontal_displacement_x_m": 5.0,
                "horizontal_displacement_z_m": -10.0,
                "horizontal_travel_m": 21.0,
                "vertical_displacement_m": 2.0,
                "vertical_min_displacement_m": -3.0,
                "vertical_max_displacement_m": 5.0
            },
            "translation_availability": "measured",
            "yaw": {
                "heading_axis": "positive_z",
                "net_yaw_deg": -90.0,
                "unwrapped_yaw_deg": 270.0,
                "yaw_travel_deg": 450.0
            },
            "yaw_availability": "measured"
        })
    );
}

#[test]
fn root_trajectory_facts_do_not_depend_on_rm_filename_or_clip_name_hint() {
    let dir = unique_temp_dir("root-trajectory-filename-policy");
    let ordinary = dir.path().join("ordinary.glb");
    let rm_named = dir.path().join("ordinary_RM.glb");
    let internal_rm_named = dir.path().join("ordinary-internal-name.glb");
    write_root_trajectory_glb(&ordinary);
    write_root_trajectory_glb(&rm_named);
    write_named_root_trajectory_glb(&internal_rm_named, "trajectory_RM");
    let config_path = write_config(dir.path(), "roles.toml", "[rig.roles]\nroot = \"root\"\n");

    let output = animsmith()
        .arg("--config")
        .arg(&config_path)
        .arg("measure")
        .arg(&ordinary)
        .arg(&rm_named)
        .arg(&internal_rm_named)
        .args(["--format", "json"])
        .output()
        .expect("measures path- and clip-named _RM copies");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    let files = json["files"].as_array().expect("three measured files");
    assert_eq!(files.len(), 3);
    let ordinary_trajectory = &files[0]["measurements"]["clips"]["trajectory"]["root_trajectory"];
    assert_eq!(
        files[0]["measurements"]["clips"]["trajectory"]["root_trajectory_availability"],
        "measured"
    );
    assert_eq!(ordinary_trajectory["source_role"], "root");
    assert_eq!(ordinary_trajectory["translation_availability"], "measured");
    assert_eq!(ordinary_trajectory["yaw_availability"], "measured");
    assert_eq!(
        files[0]["input"], files[1]["input"],
        "identical input bytes"
    );
    assert_eq!(
        *ordinary_trajectory, files[1]["measurements"]["clips"]["trajectory"]["root_trajectory"],
        "a filename policy hint must not alter content-derived root facts"
    );
    assert_eq!(
        *ordinary_trajectory, files[2]["measurements"]["clips"]["trajectory_RM"]["root_trajectory"],
        "an internal clip-name policy hint must not alter content-derived root facts"
    );
}

#[test]
fn removed_preview_format_is_rejected_as_an_operator_error() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().unwrap(),
            "--format",
            &format!("json-v2-{}", "preview"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains(&format!("invalid value 'json-v2-{}'", "preview")));
}

#[test]
fn lint_json_exposes_complete_clean_and_unselected_checks() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "nan",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["schema_version"], 17);
    assert_eq!(json["schema"], CURRENT_OUTPUT_SCHEMA_ID);
    let checks = json["files"][0]["checks"].as_array().expect("checks");
    let nan = checks
        .iter()
        .find(|check| check["check_id"] == "nan")
        .expect("nan record");
    assert_eq!(nan["selection"], "selected");
    assert_eq!(nan["configuration"], "enabled");
    assert_eq!(nan["applicability"], "applicable");
    assert_eq!(nan["evaluation"], "complete");
    assert_eq!(nan["findings"], json!([]));
    let duration = checks
        .iter()
        .find(|check| check["check_id"] == "duration-sanity")
        .expect("duration record");
    assert_eq!(duration["selection"], "unselected");
    assert_eq!(duration["evaluation"], "not_evaluated");
    let gait_group = checks
        .iter()
        .find(|check| check["check_id"] == "gait-group")
        .expect("gait-group record");
    assert_eq!(gait_group["selection"], "unselected");
    assert_eq!(gait_group["applicability"], "not_applicable");
    assert_eq!(gait_group["evaluation"], "not_evaluated");
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
}

#[test]
fn unity_generic_v2_non_fbx_input_keeps_root_motion_not_applicable() {
    let dir = unique_temp_dir("unity-generic-v2-non-fbx-root-motion");
    let config = write_config(
        dir.path(),
        "unity-generic-v2.toml",
        r#"
[engine]
profile = "unity-generic"
profile_revision = 2
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
animation_type = "generic"
avatar_setup = "create_from_this_model"
import_animation = true
root_motion_source = "root"

[clips."*".engine_settings]
root_rotation = "bake"
root_position_y = "bake"
root_position_xz = "bake"
"#,
    );

    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "engine-root-motion",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    let root_motion = lint_check(&json, "engine-root-motion");
    assert_eq!(root_motion["selection"], "selected");
    assert_eq!(root_motion["configuration"], "enabled");
    assert_eq!(root_motion["applicability"], "not_applicable");
    assert_eq!(root_motion["evaluation"], "not_evaluated");
    assert!(root_motion.get("prediction").is_none());
    assert_evaluation_summary_matches_checks(&json);
}

#[test]
fn lint_json_keeps_disabled_distinct_from_unselected() {
    let dir = unique_temp_dir("v2-disabled");
    let config = write_config(
        dir.path(),
        "disabled.toml",
        "[checks.nan]\nseverity = \"off\"\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "nan",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let checks = json["files"][0]["checks"].as_array().expect("checks");
    let nan = checks
        .iter()
        .find(|check| check["check_id"] == "nan")
        .expect("nan record");
    assert_eq!(nan["selection"], "selected");
    assert_eq!(nan["configuration"], "disabled");
    assert_eq!(nan["evaluation"], "not_evaluated");
    let duration = checks
        .iter()
        .find(|check| check["check_id"] == "duration-sanity")
        .expect("duration record");
    assert_eq!(duration["selection"], "unselected");
    assert_eq!(duration["configuration"], "enabled");
    let constant_nonunit = checks
        .iter()
        .find(|check| check["check_id"] == "constant-nonunit-scale")
        .expect("opt-in scale-channel record");
    assert_eq!(constant_nonunit["selection"], "unselected");
    assert_eq!(constant_nonunit["configuration"], "disabled");
    assert_eq!(constant_nonunit["evaluation"], "not_evaluated");
}

#[test]
fn lint_json_explicit_severity_enables_opt_in_check() {
    let dir = unique_temp_dir("v2-opt-in-scale-channel");
    let config = write_config(
        dir.path(),
        "opt-in.toml",
        "[checks.constant-nonunit-scale]\nseverity = \"note\"\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "constant-nonunit-scale",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let check = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "constant-nonunit-scale")
        .expect("opt-in scale-channel record");
    assert_eq!(check["selection"], "selected");
    assert_eq!(check["configuration"], "enabled");
    assert_eq!(check["evaluation"], "complete");
    assert_eq!(check["findings"], json!([]));
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
}

#[test]
fn lint_json_selects_non_uniform_scale_independently() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "non-uniform-scale",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let checks = json["files"][0]["checks"].as_array().expect("checks");
    let non_uniform = checks
        .iter()
        .find(|check| check["check_id"] == "non-uniform-scale")
        .expect("non-uniform scale record");
    assert_eq!(non_uniform["selection"], "selected");
    assert_eq!(non_uniform["configuration"], "enabled");
    assert_eq!(non_uniform["evaluation"], "complete");
    assert_eq!(non_uniform["findings"], json!([]));
    let scale_keys = checks
        .iter()
        .find(|check| check["check_id"] == "scale-keys")
        .expect("scale keys record");
    assert_eq!(scale_keys["selection"], "unselected");
    assert_eq!(scale_keys["evaluation"], "not_evaluated");
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
}

#[test]
fn lint_json_gait_group_can_carry_finding_and_coverage_gap() {
    let dir = unique_temp_dir("v2-partial-gait");
    let config = write_config(
        dir.path(),
        "partial.toml",
        "[gait_groups.ring]\nclips = [\"walk\", \"missing\"]\nmax_gait_phase_spread = 0.1\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "gait-group",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let gait = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "gait-group")
        .expect("gait-group record");
    assert_eq!(gait["applicability"], "applicable");
    assert_eq!(gait["evaluation"], "partial");
    assert_eq!(gait["findings"][0]["severity"], "error");
    assert_eq!(gait["findings"][0]["clip"], "missing");
    assert_eq!(gait["gaps"][0]["code"], "roles_unresolved");
    assert_eq!(gait["gaps"][0]["scope"]["code"], "phase_coherence");
    assert_eq!(gait["evaluated_scopes"][0]["code"], "member_existence");
    assert_eq!(json["summary"]["checks"]["evaluation"]["partial"], 1);
    assert_eq!(json["summary"]["checks"]["gaps"], 1);
    assert_eq!(json["summary"]["findings"]["error"], 1);
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
}

#[test]
fn lint_json_sync_group_emits_schema_valid_member_table() {
    let dir = unique_temp_dir("v7-partial-sync-group");
    let config = write_config(
        dir.path(),
        "partial.toml",
        "[clips.walk]\nloop = true\nfps = 30.0\n\n[sync_groups.ring]\nclips = [\"walk\", \"missing\"]\nmax_duration_delta_s = 0.001\nmax_frame_count_delta = 0\nmax_fps_delta = 0.01\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "sync-group",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    let sync = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "sync-group")
        .expect("sync-group record");
    assert_eq!(sync["evaluation"], "partial");
    assert_eq!(sync["findings"][0]["clip"], "missing");
    assert_eq!(sync["findings"][0]["members"][0]["member"], "walk");
    assert_eq!(
        sync["findings"][0]["members"][0]["measurements"]["availability"],
        "present"
    );
    assert_eq!(
        sync["findings"][0]["members"][1],
        serde_json::json!({
            "member": "missing",
            "measurements": { "availability": "missing" }
        })
    );
    assert!(
        sync["gaps"]
            .as_array()
            .expect("gaps")
            .iter()
            .any(|gap| gap["code"] == "insufficient_measurable_members")
    );
    assert_evaluation_summary_matches_checks(&json);
}

#[test]
fn lint_json_time_complement_emits_stable_pair_scores() {
    let dir = unique_temp_dir("v7-time-complement");
    let input = dir.path().join("time-complement.glb");
    write_time_complement_glb(&input);
    let config = write_config(
        dir.path(),
        "time-complement.toml",
        "[sync_groups.ring]\nclips = [\"forward\", \"reflected\"]\nmax_duration_delta_s = 0.001\nmax_frame_count_delta = 0\nmax_fps_delta = 0.01\n\n[sync_groups.ring.time_complement]\nmin_reflected_time_advantage = 0.25\nmin_lr_amplitude_m = 0.03\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .arg("lint")
        .arg(&input)
        .args([
            "--format",
            "json",
            "--select",
            "time-complement",
            "--deny-warnings",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    let check = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "time-complement")
        .expect("time-complement record");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "complete");
    assert!(check.get("gaps").is_none(), "empty gaps are omitted");

    let finding = &check["findings"][0];
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["expected"], 0.25);
    assert!(
        finding["message"]
            .as_str()
            .unwrap()
            .contains("'forward' and 'reflected'")
    );
    let members = finding["members"].as_array().expect("pair member rows");
    assert_eq!(members.len(), 2);
    assert_eq!(members[0]["member"], "forward");
    assert_eq!(members[1]["member"], "reflected");
    for member in members {
        let measurements = member["measurements"].as_object().expect("measurements");
        assert_eq!(
            measurements.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "gait_phase",
                "lr_amplitude_m",
                "reflected_time_advantage",
                "reflected_time_similarity",
                "same_time_similarity",
            ]
        );
    }
    let scores = members[0]["measurements"].as_object().unwrap();
    for score in [
        "same_time_similarity",
        "reflected_time_similarity",
        "reflected_time_advantage",
    ] {
        assert_eq!(
            members[0]["measurements"][score],
            members[1]["measurements"][score]
        );
    }
    let same = scores["same_time_similarity"].as_f64().unwrap();
    let reflected = scores["reflected_time_similarity"].as_f64().unwrap();
    let advantage = scores["reflected_time_advantage"].as_f64().unwrap();
    assert!(reflected > same);
    assert!((advantage - (reflected - same)).abs() < 1e-12);
    assert_eq!(finding["measured"].as_f64().unwrap(), advantage);
    assert_evaluation_summary_matches_checks(&json);
}

#[test]
fn lint_json_exit_policy_uses_findings_not_coverage_gaps() {
    let warning_dir = unique_temp_dir("v2-warning-exit");
    let warning_input = warning_dir.path().join("flipped.glb");
    write_flipped_glb(&warning_input);

    for (deny, expected) in [(false, 0), (true, 1)] {
        let mut args = vec![
            "lint",
            warning_input.to_str().expect("utf-8 input path"),
            "--format",
            "json",
            "--select",
            "quat-flip",
        ];
        if deny {
            args.push("--deny-warnings");
        }
        let output = animsmith().args(&args).output().expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(expected),
            "warning exit (deny-warnings: {deny}):\n{}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        let quat_flip = json["files"][0]["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .find(|check| check["check_id"] == "quat-flip")
            .expect("quat-flip record");
        assert_eq!(quat_flip["findings"][0]["severity"], "warning");
        assert!(quat_flip["gaps"].is_null());
    }

    let gap_dir = unique_temp_dir("v2-gap-exit");
    let gap_input = gap_dir.path().join("sway.glb");
    write_clean_glb(&gap_input);
    let config = write_config(gap_dir.path(), "gap.toml", "[clips.sway]\nloop = true\n");
    for deny in [false, true] {
        let mut args = vec![
            "--config",
            config.to_str().expect("utf-8 config path"),
            "lint",
            gap_input.to_str().expect("utf-8 input path"),
            "--format",
            "json",
            "--select",
            "loop-seam",
        ];
        if deny {
            args.push("--deny-warnings");
        }
        let output = animsmith().args(&args).output().expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(0),
            "coverage gap must not gate (deny-warnings: {deny}):\n{}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        let loop_seam = json["files"][0]["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .find(|check| check["check_id"] == "loop-seam")
            .expect("loop-seam record");
        assert_eq!(loop_seam["findings"], json!([]));
        assert_eq!(loop_seam["gaps"][0]["code"], "roles_unresolved");
    }
}

#[test]
fn lint_json_rejects_allow_instead_of_deleting_evidence() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--allow",
            "nan",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("machine-readable results retain every content finding"));
}

#[test]
fn diff_json_uses_versioned_envelope() {
    let path = fixture("rig.gltf");
    let output = animsmith()
        .args([
            "diff",
            path.to_str().expect("utf-8 fixture path"),
            path.to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    assert_eq!(json["schema_version"], 17);
    assert_eq!(json["schema"], CURRENT_OUTPUT_SCHEMA_ID);
    assert_eq!(json["tool"]["name"], "animsmith");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["command"], "diff");
    assert_eq!(json["summary"]["deltas"], 0);
    assert_eq!(json["deltas"].as_array().expect("deltas array").len(), 0);
    assert!(json["inputs"]["before"].is_string());
    assert!(json["inputs"]["after"].is_string());
}

#[test]
fn output_schema_rejects_cross_command_and_nested_contract_drift() {
    let output = animsmith()
        .args([
            "measure",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let measure: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&measure);
    let validator = output_validator();

    let mut foreign_field = measure.clone();
    foreign_field["deltas"] = json!([]);
    assert!(!validator.is_valid(&foreign_field));

    let mut nested_version = measure.clone();
    nested_version["files"][0]["measurements"]["schema_version"] = json!(7);
    assert!(!validator.is_valid(&nested_version));

    let mut missing_input = measure.clone();
    missing_input["files"][0]
        .as_object_mut()
        .expect("file record")
        .remove("input");
    assert!(!validator.is_valid(&missing_input));

    let mut uppercase_digest = measure.clone();
    uppercase_digest["files"][0]["input"]["sha256"] = json!("A".repeat(64));
    assert!(!validator.is_valid(&uppercase_digest));

    let mut negative_byte_count = measure.clone();
    negative_byte_count["files"][0]["input"]["bytes"] = json!(-1);
    assert!(!validator.is_valid(&negative_byte_count));

    let mut lint_without_checks = measure;
    lint_without_checks["command"] = json!("lint");
    assert!(!validator.is_valid(&lint_without_checks));
}

#[test]
fn diff_accepts_single_file_measure_report_round_trip() {
    let dir = unique_temp_dir("diff-round-trip");
    let asset = fixture("rig.gltf");
    let report_path = dir.path().join("measure.json");

    let measured = animsmith()
        .args([
            "measure",
            asset.to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(measured.status.success(), "stderr:\n{}", stderr(&measured));
    std::fs::write(&report_path, &measured.stdout).expect("writes report");

    // A report diffed against the asset it was measured from is clean.
    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            asset.to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");
    // Clean == exit 0; the "no significant movement" prose is not the
    // contract (that's the exit code) and is left unpinned.
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn diff_keeps_regeneration_guidance_for_finite_json_outside_f32() {
    let dir = unique_temp_dir("diff-f32-overflow-guidance");
    let asset = fixture("rig.gltf");
    let report_path = dir.path().join("measure.json");

    let measured = animsmith()
        .args([
            "measure",
            asset.to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(measured.status.success(), "stderr:\n{}", stderr(&measured));
    let mut report: Value =
        serde_json::from_slice(&measured.stdout).expect("measurement output is JSON");
    report["files"][0]["measurements"]["skeleton_nodes"][0]["local_rest"]["translation_parent_space_m"]
        [0] = json!(1e39_f64);
    write_json(&report_path, &report);

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            asset.to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let error = stderr(&output);
    assert!(
        error.contains("must be finite; regenerate it from the original asset"),
        "stderr:\n{error}"
    );
    assert!(!error.contains("bad JSON"), "stderr:\n{error}");
    assert!(!error.contains("number out of range"), "stderr:\n{error}");
}

#[test]
fn diff_accepts_single_file_lint_report_round_trip() {
    let dir = unique_temp_dir("diff-lint-round-trip");
    let asset = fixture("rig.gltf");
    let report_path = dir.path().join("lint.json");
    let linted = animsmith()
        .args([
            "lint",
            asset.to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(linted.status.success(), "stderr:\n{}", stderr(&linted));
    std::fs::write(&report_path, &linted.stdout).expect("writes report");

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            asset.to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn diff_accepts_measurement_json_and_exits_one_for_deltas() {
    let dir = unique_temp_dir("diff-json-deltas");
    let before = dir.path().join("before.json");
    let after = dir.path().join("after.json");
    write_json(&before, &measurement_report(1.0));
    write_json(&after, &measurement_report(1.1));

    let output = animsmith()
        .args([
            "diff",
            before.to_str().expect("utf-8 before path"),
            after.to_str().expect("utf-8 after path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    // The CLI contract is the envelope shape + exit code: one delta,
    // routed to its clip. The metric/note strings are the unit suite's
    // job (diff.rs), so they are not re-pinned here.
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["summary"]["deltas"].as_u64(), Some(1));
    assert_eq!(json["deltas"][0]["clip"], "walk");
}

#[test]
fn diff_accepts_measurement_json_and_exits_zero_without_deltas() {
    let dir = unique_temp_dir("diff-json-clean");
    let before = dir.path().join("before.json");
    let after = dir.path().join("after.json");
    let report = measurement_report(1.0);
    write_json(&before, &report);
    write_json(&after, &report);

    let output = animsmith()
        .args([
            "diff",
            before.to_str().expect("utf-8 before path"),
            after.to_str().expect("utf-8 after path"),
        ])
        .output()
        .expect("runs animsmith");

    // Identical reports in, exit 0 out — the exit code is the contract,
    // not the human-format prose.
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
}

#[test]
fn diff_rejects_zero_or_multiple_measurement_file_records() {
    let dir = unique_temp_dir("diff-report-file-count");
    let report_path = dir.path().join("report.json");

    for file_count in [0, 2, 3, 10] {
        let mut report = measurement_report(1.0);
        let file = report["files"][0].clone();
        report["files"] = Value::Array(vec![file; file_count]);
        write_json(&report_path, &report);

        let output = animsmith()
            .args([
                "diff",
                report_path.to_str().expect("utf-8 report path"),
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            ])
            .output()
            .expect("runs animsmith");

        assert_eq!(
            output.status.code(),
            Some(2),
            "stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        assert!(stdout(&output).is_empty());
        assert_eq!(
            stderr(&output),
            format!(
                "animsmith: {} contains {file_count} file records; diff expects a single-file measurement report\n",
                report_path.display()
            )
        );
    }
}

#[test]
fn diff_preserves_error_precedence_for_malformed_multi_file_reports() {
    let dir = unique_temp_dir("diff-malformed-multi-report");
    let report_path = dir.path().join("report.json");
    let base = measurement_report(1.0);
    let file = base["files"][0].clone();

    let mut invalid_first = base.clone();
    invalid_first["files"] = json!([file.clone(), file.clone()]);
    invalid_first["files"][0]
        .as_object_mut()
        .expect("file record")
        .remove("measurements");

    let mut invalid_second = base.clone();
    invalid_second["files"] = json!([file.clone(), file.clone()]);
    invalid_second["files"][1]
        .as_object_mut()
        .expect("file record")
        .remove("measurements");

    let mut missing_command = base.clone();
    missing_command["files"] = json!([file.clone(), file.clone()]);
    missing_command
        .as_object_mut()
        .expect("report envelope")
        .remove("command");

    let mut wrong_output_identity = base.clone();
    wrong_output_identity["files"] = json!([file.clone(), file.clone()]);
    wrong_output_identity["schema"] = json!("urn:other:output");

    let mut invalid_third = base;
    invalid_third["files"] = json!([file.clone(), file.clone(), file]);
    invalid_third["files"][2]["measurements"]["schema"] = json!("urn:other:measurements");

    let remediation =
        "regenerate it from the original asset with `animsmith measure --format json <asset>`";
    for (name, report, expected) in [
        (
            "invalid first record",
            invalid_first,
            "contains 2 file records; diff expects a single-file measurement report".to_owned(),
        ),
        (
            "invalid second record",
            invalid_second,
            "contains 2 file records; diff expects a single-file measurement report".to_owned(),
        ),
        (
            "invalid third record",
            invalid_third,
            "contains 3 file records; diff expects a single-file measurement report".to_owned(),
        ),
        (
            "missing envelope command",
            missing_command,
            format!("is not an animsmith measurement report (no `command`); {remediation}"),
        ),
        (
            "wrong output identity",
            wrong_output_identity,
            format!("does not identify output contract {CURRENT_OUTPUT_SCHEMA_ID}; {remediation}"),
        ),
    ] {
        write_json(&report_path, &report);
        let output = animsmith()
            .args([
                "diff",
                report_path.to_str().expect("utf-8 report path"),
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            ])
            .output()
            .expect("runs animsmith");

        assert_eq!(output.status.code(), Some(2), "{name}");
        assert!(stdout(&output).is_empty(), "{name}");
        assert_eq!(
            stderr(&output),
            format!("animsmith: {} {expected}\n", report_path.display()),
            "{name}"
        );
    }
}

#[test]
fn diff_preserves_tailored_report_errors_and_remediation() {
    let dir = unique_temp_dir("diff-report-errors");
    let report_path = dir.path().join("report.json");
    let base = measurement_report(1.0);
    let without = |pointer: &str| {
        let mut report = base.clone();
        let (parent, key) = pointer.rsplit_once('/').expect("JSON pointer has a key");
        let object = if parent.is_empty() {
            &mut report
        } else {
            report.pointer_mut(parent).expect("fixture path exists")
        };
        object
            .as_object_mut()
            .expect("path ends at an object")
            .remove(key);
        report
    };
    let mut unsupported_output_version = base.clone();
    unsupported_output_version["schema_version"] = json!(2);
    let mut wrong_output_identity = base.clone();
    wrong_output_identity["schema"] = json!("urn:other:output");
    let mut unsupported_command = base.clone();
    unsupported_command["command"] = json!("diff");
    let mut unsupported_measurement_version = base.clone();
    unsupported_measurement_version["files"][0]["measurements"]["schema_version"] = json!(7);
    let mut wrong_measurement_identity = base.clone();
    wrong_measurement_identity["files"][0]["measurements"]["schema"] =
        json!("urn:other:measurements");

    let remediation =
        "regenerate it from the original asset with `animsmith measure --format json <asset>`";
    let cases = vec![
        (
            "missing output version",
            without("/schema_version"),
            format!("is not an animsmith report envelope (no `schema_version`); {remediation}"),
        ),
        (
            "wrong output identity",
            wrong_output_identity,
            format!("does not identify output contract {CURRENT_OUTPUT_SCHEMA_ID}; {remediation}"),
        ),
        (
            "unsupported output version",
            unsupported_output_version,
            format!("has schema_version 2; this build reads schema_version 17; {remediation}"),
        ),
        (
            "missing command",
            without("/command"),
            format!("is not an animsmith measurement report (no `command`); {remediation}"),
        ),
        (
            "unsupported command",
            unsupported_command,
            "is a \"diff\" report; diff reads only measure or lint reports".to_owned(),
        ),
        (
            "missing files",
            without("/files"),
            format!("is not an animsmith report envelope (no `files` array); {remediation}"),
        ),
        (
            "missing path",
            without("/files/0/path"),
            format!("report file record has no `path`; {remediation}"),
        ),
        (
            "missing measurements",
            without("/files/0/measurements"),
            "report has no measurements".to_owned(),
        ),
        (
            "missing measurement version",
            without("/files/0/measurements/schema_version"),
            format!("has no versioned measurement contract; {remediation}"),
        ),
        (
            "unsupported measurement version",
            unsupported_measurement_version,
            format!(
                "has measurement schema_version 7; this build reads measurement schema_version 16; {remediation}"
            ),
        ),
        (
            "wrong measurement identity",
            wrong_measurement_identity,
            format!(
                "does not identify measurement contract {MEASUREMENTS_SCHEMA_ID}; {remediation}"
            ),
        ),
        (
            "missing clips",
            without("/files/0/measurements/clips"),
            "measurement contract has no `clips` map".to_owned(),
        ),
    ];

    for (name, report, expected) in cases {
        write_json(&report_path, &report);
        let output = animsmith()
            .args([
                "diff",
                report_path.to_str().expect("utf-8 report path"),
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            ])
            .output()
            .expect("runs animsmith");

        assert_eq!(output.status.code(), Some(2), "{name}");
        assert!(stdout(&output).is_empty(), "{name}");
        assert_eq!(
            stderr(&output),
            format!("animsmith: {} {expected}\n", report_path.display()),
            "{name}"
        );
    }
}

#[test]
fn diff_compares_decoded_numbers_not_json_lexical_spelling() {
    let dir = unique_temp_dir("diff-json-number-spelling");
    let before = dir.path().join("before.json");
    let after = dir.path().join("after.json");
    let decimal = serde_json::to_string(&measurement_report(1.0)).unwrap();
    assert!(decimal.contains("\"duration_s\":1.0,"));
    let integer = decimal.replace("\"duration_s\":1.0,", "\"duration_s\":1,");
    let exponent = decimal.replace("\"duration_s\":1.0,", "\"duration_s\":1e0,");

    for (left, right) in [
        (&integer, &decimal),
        (&decimal, &integer),
        (&exponent, &decimal),
    ] {
        std::fs::write(&before, left).unwrap();
        std::fs::write(&after, right).unwrap();
        let output = animsmith()
            .args([
                "diff",
                before.to_str().unwrap(),
                after.to_str().unwrap(),
                "--format",
                "json",
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr:\n{}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(json["summary"]["deltas"], 0);
    }
}

#[test]
fn diff_rejects_alpha_v1_reports() {
    let dir = unique_temp_dir("diff-v1-report");
    let old = dir.path().join("v1.json");
    let mut report = measurement_report(1.0);
    report["schema_version"] = json!(1);
    report["schema"] = json!("urn:animsmith:schema:output:1");
    write_json(&old, &report);

    let output = animsmith()
        .args([
            "diff",
            old.to_str().unwrap(),
            fixture("rig.gltf").to_str().unwrap(),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("schema_version 1"));
}

#[test]
fn diff_rejects_outer_and_nested_contract_identity_drift() {
    let dir = unique_temp_dir("diff-contract-identity");
    let report_path = dir.path().join("report.json");
    let cases = [
        (
            {
                let mut report = measurement_report(1.0);
                report["schema"] = json!("urn:animsmith:schema:output:other");
                report
            },
            "does not identify output contract",
        ),
        (
            {
                let mut report = measurement_report(1.0);
                report["files"][0]["measurements"]["schema_version"] = json!(7);
                report
            },
            "has measurement schema_version 7; this build reads measurement schema_version 16; regenerate it from the original asset with `animsmith measure --format json <asset>`",
        ),
        (
            {
                let mut report = measurement_report(1.0);
                report["files"][0]["measurements"]["schema"] =
                    json!("urn:animsmith:schema:measurements:other");
                report
            },
            "does not identify measurement contract",
        ),
    ];
    for (report, expected) in cases {
        write_json(&report_path, &report);
        let output = animsmith()
            .args([
                "diff",
                report_path.to_str().unwrap(),
                fixture("rig.gltf").to_str().unwrap(),
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(output.status.code(), Some(2));
        assert!(stdout(&output).is_empty());
        assert!(
            stderr(&output).contains(expected),
            "stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn diff_rejects_non_measurement_report_commands() {
    let dir = unique_temp_dir("diff-wrong-command");
    let report_path = dir.path().join("diff.json");
    let mut report = measurement_report(1.0);
    report["command"] = json!("diff");
    write_json(&report_path, &report);

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().unwrap(),
            fixture("rig.gltf").to_str().unwrap(),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("reads only measure or lint reports"));
}

#[test]
fn diff_text_format_renders_deltas_and_clean_summary() {
    // The default (human) format has its own render branch that the JSON
    // contract tests never exercise. This is the one test that owns that
    // branch: a dirty diff must name the moved clip and print a change
    // summary; a clean diff must print its clean line. (The envelope /
    // exit-code contract tests deliberately do NOT string-match this
    // prose — pinning the renderer is this test's job, not theirs.)
    let dir = unique_temp_dir("diff-text-format");
    let before = dir.path().join("before.json");
    let after = dir.path().join("after.json");
    write_json(&before, &measurement_report(1.0));
    write_json(&after, &measurement_report(1.1));

    let dirty = animsmith()
        .args([
            "diff",
            before.to_str().expect("utf-8 before path"),
            after.to_str().expect("utf-8 after path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(dirty.status.code(), Some(1), "stderr:\n{}", stderr(&dirty));
    let out = stdout(&dirty);
    assert!(
        out.contains("walk"),
        "dirty Text output names the clip:\n{out}"
    );
    assert!(
        out.contains("significant change"),
        "dirty Text output summarizes the change count:\n{out}"
    );

    let clean = animsmith()
        .args([
            "diff",
            before.to_str().expect("utf-8 before path"),
            before.to_str().expect("utf-8 before path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(clean.status.code(), Some(0), "stderr:\n{}", stderr(&clean));
    assert!(
        stdout(&clean).contains("no significant movement"),
        "clean Text output states no movement:\n{}",
        stdout(&clean)
    );
}

#[test]
fn diff_text_escapes_controls_from_report_clip_metric_and_note_fields() {
    let dir = unique_temp_dir("diff-text-controls");
    let before_path = dir.path().join("before.json");
    let after_path = dir.path().join("after.json");
    let hostile = "forged\nline\u{1b}[31m";
    let mut before = measurement_report(1.0);
    let mut after = measurement_report(1.1);
    for report in [&mut before, &mut after] {
        let clip = report["files"][0]["measurements"]["clips"]
            .as_object_mut()
            .expect("clip map")
            .remove("walk")
            .expect("walk fixture");
        report["files"][0]["measurements"]["clips"]
            .as_object_mut()
            .expect("clip map")
            .insert(hostile.into(), clip);
        report["files"][0]["measurements"]["clips"][hostile]["animated_bones"] = json!([hostile]);
        report["files"][0]["measurements"]["clips"][hostile]["bone_channels"] = json!([{
            "bone_index": 0,
            "bone_name": hostile,
            "properties": ["translation"]
        }]);
    }
    before["files"][0]["measurements"]["clips"][hostile]["bone_rotation_range_deg"] = json!({});
    before["files"][0]["measurements"]["clips"][hostile]["bone_rotation_range_deg"]
        .as_object_mut()
        .expect("rotation map")
        .insert(hostile.into(), json!(0.0));
    after["files"][0]["measurements"]["clips"][hostile]["bone_rotation_range_deg"] = json!({});
    after["files"][0]["measurements"]["clips"][hostile]["bone_rotation_range_deg"]
        .as_object_mut()
        .expect("rotation map")
        .insert(hostile.into(), json!(10.0));
    write_json(&before_path, &before);
    write_json(&after_path, &after);

    let output = animsmith()
        .args(["diff"])
        .arg(&before_path)
        .arg(&after_path)
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let text = stdout(&output);
    assert!(!text.contains(hostile), "raw controls leaked:\n{text}");
    assert!(text.contains("\\n"), "newline not escaped:\n{text}");
    assert!(text.contains("\\u{1b}"), "escape not escaped:\n{text}");
}

#[test]
fn diff_rejects_json_without_schema_version_with_measure_remediation() {
    let dir = unique_temp_dir("diff-bare-map");
    let bare = dir.path().join("bare.json");
    // A bare measurement map (a pre-publish development shape) has no
    // schema_version and must be rejected with regenerate guidance.
    std::fs::write(&bare, r#"{"walk": {"duration_s": 1.0}}"#).expect("writes bare map");

    let output = animsmith()
        .args([
            "diff",
            bare.to_str().expect("utf-8 path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("not an animsmith report envelope"),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains(
            "regenerate it from the original asset with `animsmith measure --format json <asset>`"
        ),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn diff_rejects_unsupported_schema_versions() {
    let dir = unique_temp_dir("diff-future-schema");
    let future = dir.path().join("future.json");
    for version in [2, 3, 5, 8, 99] {
        let mut report = measurement_report(1.0);
        report["schema_version"] = json!(version);
        write_json(&future, &report);
        let output = animsmith()
            .args([
                "diff",
                future.to_str().expect("utf-8 path"),
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(2),
            "stdout:\n{}",
            stdout(&output)
        );
        assert!(
            stderr(&output).contains(&format!("schema_version {version}")),
            "stderr:\n{}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains(
                "regenerate it from the original asset with `animsmith measure --format json <asset>`"
            ),
            "version {version}: stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn diff_rejects_historical_output_v5_with_v11_measurements() {
    let dir = unique_temp_dir("diff-output-v5-measurements-v11");
    let report_path = dir.path().join("historical.json");
    let mut report = measurement_report(1.0);
    report["schema_version"] = json!(5);
    report["schema"] = json!("urn:animsmith:schema:output:5");
    report["files"][0]["measurements"]["schema_version"] = json!(11);
    report["files"][0]["measurements"]["schema"] = json!("urn:animsmith:schema:measurements:11");
    write_json(&report_path, &report);

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains(
            "has schema_version 5; this build reads schema_version 17; regenerate it from the original asset with `animsmith measure --format json <asset>`"
        ),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn diff_rejects_all_unsupported_nested_measurement_schema_versions() {
    let dir = unique_temp_dir("diff-unsupported-nested-schema");
    let report_path = dir.path().join("report.json");
    for version in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 99] {
        let mut report = measurement_report(1.0);
        report["files"][0]["measurements"]["schema_version"] = json!(version);
        write_json(&report_path, &report);
        let output = animsmith()
            .args([
                "diff",
                report_path.to_str().expect("utf-8 path"),
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(2),
            "version {version}: stdout:\n{}",
            stdout(&output)
        );
        assert!(
            stderr(&output).contains(&format!(
                "has measurement schema_version {version}; this build reads measurement schema_version 16"
            )),
            "version {version}: stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn diff_rejects_v11_skeleton_shape_before_decoding_v13_fields() {
    let dir = unique_temp_dir("diff-v11-skeleton-shape");
    let report_path = dir.path().join("report.json");
    let mut report = measurement_report(1.0);
    report["files"][0]["measurements"]["schema_version"] = json!(11);
    report["files"][0]["measurements"]["schema"] = json!("urn:animsmith:schema:measurements:11");
    report["files"][0]["measurements"]["skeleton_source_coverage"] = json!("complete");
    report["files"][0]["measurements"]["skeleton_nodes"] = json!([{
        "node_index": 0,
        "scene_root_indices": [],
        "local_rest": {
            "kind": "trs",
            "translation_m": [0.0, 0.0, 0.0],
            "rotation_xyzw": [0.0, 0.0, 0.0, 1.0],
            "scale": [1.0, 1.0, 1.0]
        },
        "rest_world_matrix": [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0
        ]
    }]);
    write_json(&report_path, &report);

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains(
            "has measurement schema_version 11; this build reads measurement schema_version 16; regenerate it from the original asset with `animsmith measure --format json <asset>`"
        ),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!stderr(&output).contains("bad JSON"));
}

#[test]
fn diff_does_not_accept_v10_skeleton_or_skin_shapes_under_the_v13_identity() {
    let dir = unique_temp_dir("diff-v11-shape-v13-identity");
    let report_path = dir.path().join("report.json");
    let mut report = measurement_report(1.0);
    report["files"][0]["measurements"]["skeleton_source_coverage"] = json!("complete");
    report["files"][0]["measurements"]["skeleton_nodes"] = json!([{
        "node_index": 0,
        "scene_root_indices": [],
        "local_rest": {
            "kind": "trs",
            "translation_m": [0.0, 0.0, 0.0],
            "rotation_xyzw": [0.0, 0.0, 0.0, 1.0],
            "scale": [1.0, 1.0, 1.0]
        },
        "rest_world_matrix": [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0
        ]
    }]);
    write_json(&report_path, &report);

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains(
            "measurement structure skeleton_nodes[0] is invalid: uses a shape from an earlier measurement contract"
        ),
        "stderr:\n{}",
        stderr(&output)
    );

    report["files"][0]["measurements"]["skeleton_source_coverage"] = json!("unavailable");
    report["files"][0]["measurements"]["skeleton_nodes"] = json!([]);
    report["files"][0]["measurements"]["skins"] = json!([{ "skin_index": 0 }]);
    write_json(&report_path, &report);
    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains(
            "measurement structure skins[0] is invalid: uses a shape from an earlier measurement contract"
        ),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn diff_rejects_envelope_without_files() {
    let dir = unique_temp_dir("diff-no-files");
    let report = dir.path().join("no-files.json");
    write_json(
        &report,
        &json!({
            "schema_version": 13,
            "schema": OUTPUT_V13_SCHEMA_ID,
            "tool": {
                "name": "animsmith",
                "version": env!("CARGO_PKG_VERSION"),
                "source": { "revision": null, "dirty": null }
            },
            "command": "measure",
            "summary": { "files": 0 }
        }),
    );

    let output = animsmith()
        .args([
            "diff",
            report.to_str().expect("utf-8 path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("no `files` array"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn lint_counts_severities_in_summary_and_text() {
    let dir = unique_temp_dir("lint-severity-counts");
    let input = dir.path().join("dirty.glb");
    write_flipped_glb(&input);

    // JSON: the flipped fixture produces exactly one quat-flip warning;
    // the summary must bucket it as a warning, not a note or error.
    let output = animsmith()
        .args([
            "lint",
            input.to_str().expect("utf-8 input path"),
            "--format",
            "json",
            "--select",
            "quat-flip",
        ])
        .output()
        .expect("runs animsmith");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["summary"]["findings"]["warning"], 1, "{json:#}");
    assert_eq!(json["summary"]["findings"]["error"], 0, "{json:#}");
    assert_eq!(json["summary"]["findings"]["note"], 0, "{json:#}");
    let quat_flip = json["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["check_id"] == "quat-flip")
        .unwrap();
    assert_eq!(quat_flip["findings"][0]["severity"], "warning");

    // Text mode counts through the same severity match.
    let output = animsmith()
        .args([
            "lint",
            input.to_str().expect("utf-8 input path"),
            "--select",
            "quat-flip",
        ])
        .output()
        .expect("runs animsmith");
    assert!(
        stdout(&output).contains("1 warning(s)"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn fix_reports_unreadable_input_as_operator_error() {
    let output = animsmith()
        .args(["fix", "missing.glb", "--dry-run"])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("failed to read"),
        "stderr:\n{}",
        stderr(&output)
    );
}

/// 3 keyframe times but 2 output values — structurally malformed.
const COUNT_MISMATCH_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,AAAAAAAAAD8AAIA/AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8=", "byteLength": 44 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 12 },
    { "buffer": 0, "byteOffset": 12, "byteLength": 32 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "bad",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

/// First keyframe time is NaN; values are valid identity quats.
const NAN_TIME_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,AADAfwAAAD8AAIA/AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAAAAAAAAAAAAAAAIA/", "byteLength": 60 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 12 },
    { "buffer": 0, "byteOffset": 12, "byteLength": 48 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "poisoned",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

/// First and last keyframe times are NaN and +Inf; values remain valid.
const NONFINITE_TIME_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,AADAfwAAAD8AAIB/AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAAAAAAAAAAAAAAAIA/", "byteLength": 60 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 12 },
    { "buffer": 0, "byteOffset": 12, "byteLength": 48 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "poisoned",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

#[test]
fn malformed_track_counts_are_operator_errors_everywhere() {
    let dir = unique_temp_dir("count-mismatch-cli");
    let input = dir.path().join("bad.gltf");
    std::fs::write(&input, COUNT_MISMATCH_GLTF).expect("writes fixture");
    let out = dir.path().join("out.glb");

    let commands: [&[&str]; 3] = [
        &["measure", input.to_str().expect("utf-8 path")],
        &["lint", input.to_str().expect("utf-8 path")],
        &[
            "transform",
            input.to_str().expect("utf-8 path"),
            "-o",
            out.to_str().expect("utf-8 path"),
        ],
    ];
    for args in commands {
        let output = animsmith().args(args).output().expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        assert!(
            stderr(&output).contains("malformed animation data"),
            "{args:?}: stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn nan_key_times_lint_as_errors_and_never_crash() {
    let dir = unique_temp_dir("nan-time-cli");
    let input = dir.path().join("nan.gltf");
    std::fs::write(&input, NAN_TIME_GLTF).expect("writes fixture");

    // measure survives (exit 0): NaN is a semantic defect for lint to
    // judge, not a crash.
    let output = animsmith()
        .args(["measure", input.to_str().expect("utf-8 path")])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    // lint reports the nan error finding and exits 1.
    let output = animsmith()
        .args(["lint", input.to_str().expect("utf-8 path")])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("error[nan]") && stdout(&output).contains("non-finite key time"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn non_finite_key_times_never_escape_as_schema_invalid_nulls() {
    let dir = unique_temp_dir("nonfinite-time-json");
    let input = dir.path().join("nonfinite.gltf");
    std::fs::write(&input, NONFINITE_TIME_GLTF).expect("writes fixture");

    for (command, expected_exit) in [("measure", 0), ("lint", 1)] {
        let output = animsmith()
            .args([command, input.to_str().unwrap(), "--format", "json"])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "{command} stderr:\n{}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        assert_output_schema_valid(&json);
        assert_eq!(
            json["files"][0]["measurements"]["clips"]["poisoned"]["duration_s"],
            0.5
        );
    }
}

/// Every `--format json` path in this CLI treats a stdout it cannot write to
/// the same way: the write failure is diagnosed on stderr and the command's
/// own outcome code stands.
///
/// `lint` is the load-bearing case. Findings it really found must not be
/// relabelled as an operator error because the consumer hung up — `lint …
/// --format json | head` still exits `1`. `diff`'s exit `1` for movement it
/// really measured is the same claim.
///
/// The pipe's read end is dropped **before** the child is spawned, so its
/// stdout has no reader from the moment it exists: the write failure is a
/// property of the setup rather than a race against how quickly the child
/// reaches its write.
#[test]
fn a_closed_stdout_is_diagnosed_without_rewriting_any_json_command_outcome() {
    #[cfg(feature = "fbx")]
    let dir = unique_temp_dir("closed-stdout-json");

    #[cfg_attr(not(feature = "fbx"), expect(unused_mut))]
    let mut cases: Vec<(&str, Vec<String>, i32)> = vec![
        (
            "measure",
            vec![example_asset("clip.glb").display().to_string()],
            0,
        ),
        (
            "lint",
            vec![example_asset("clip-dirty.glb").display().to_string()],
            1,
        ),
        (
            "diff",
            vec![
                example_asset("clip.glb").display().to_string(),
                example_asset("walk.glb").display().to_string(),
            ],
            1,
        ),
    ];
    #[cfg(feature = "fbx")]
    cases.push((
        "convert",
        vec![
            example_asset("clip.glb").display().to_string(),
            "-o".to_owned(),
            dir.path().join("converted.glb").display().to_string(),
        ],
        0,
    ));

    for (command, args, expected_exit) in cases {
        let (reader, writer) = std::io::pipe().expect("creates a pipe");
        drop(reader);
        let output = animsmith()
            .arg(command)
            .args(&args)
            .args(["--format", "json"])
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawns animsmith {command}: {error}"))
            .wait_with_output()
            .unwrap_or_else(|error| panic!("waits for animsmith {command}: {error}"));

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "{command} must keep its own outcome when stdout is closed; stderr:\n{stderr}"
        );
        // Ours, not the OS's: the platform's wording for a reader-less pipe
        // is not this contract.
        assert_eq!(
            stderr
                .matches("animsmith: cannot write JSON output to stdout")
                .count(),
            1,
            "{command} stderr:\n{stderr}"
        );
    }
}

/// Human-readable output follows the same reporting boundary as JSON: losing
/// stdout is diagnosed, never panicked over, and never substitutes exit `2`
/// for the command's own success or finding/refusal status.
///
/// This matrix includes iterator-shaped renderers (`inspect`, `measure`, and
/// `diff`), whole-result text and Markdown (`lint`), and write-summary paths
/// (`transform` and, when enabled, `report`). The non-feature-gated cases are
/// also run by the required `--no-default-features` CLI gate.
#[test]
fn a_closed_stdout_preserves_text_and_markdown_command_outcomes() {
    let dir = unique_temp_dir("closed-stdout-text");
    let clean = example_asset("clip.glb").display().to_string();
    let dirty = example_asset("clip-dirty.glb").display().to_string();
    let other = example_asset("walk.glb").display().to_string();

    #[cfg_attr(not(feature = "report"), expect(unused_mut))]
    let mut cases: Vec<(&str, Vec<String>, i32)> = vec![
        ("inspect", vec!["inspect".to_owned(), clean.clone()], 0),
        (
            "measure text",
            vec![
                "measure".to_owned(),
                clean.clone(),
                "--format".to_owned(),
                "text".to_owned(),
            ],
            0,
        ),
        (
            "lint text success",
            vec![
                "lint".to_owned(),
                clean.clone(),
                "--format".to_owned(),
                "text".to_owned(),
            ],
            0,
        ),
        (
            "lint markdown success",
            vec![
                "lint".to_owned(),
                clean.clone(),
                "--format".to_owned(),
                "markdown".to_owned(),
            ],
            0,
        ),
        (
            "lint text refusal",
            vec![
                "lint".to_owned(),
                dirty.clone(),
                "--format".to_owned(),
                "text".to_owned(),
            ],
            1,
        ),
        (
            "lint markdown refusal",
            vec![
                "lint".to_owned(),
                dirty,
                "--format".to_owned(),
                "markdown".to_owned(),
            ],
            1,
        ),
        (
            "diff refusal",
            vec![
                "diff".to_owned(),
                clean.clone(),
                other,
                "--format".to_owned(),
                "text".to_owned(),
            ],
            1,
        ),
        (
            "transform summary",
            vec![
                "transform".to_owned(),
                clean.clone(),
                "-o".to_owned(),
                dir.path().join("transformed.glb").display().to_string(),
                "--hold-extend".to_owned(),
                "0.1".to_owned(),
            ],
            0,
        ),
    ];
    #[cfg(feature = "report")]
    cases.push((
        "report summary",
        vec![
            "report".to_owned(),
            clean,
            "-o".to_owned(),
            dir.path().join("report.html").display().to_string(),
        ],
        0,
    ));

    for (case, args, expected_exit) in cases {
        let (reader, writer) = std::io::pipe().expect("creates a pipe");
        drop(reader);
        let output = animsmith()
            .args(&args)
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawns {case}: {error}"))
            .wait_with_output()
            .unwrap_or_else(|error| panic!("waits for {case}: {error}"));

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "{case} must keep its own outcome when stdout is closed; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr
                .matches("animsmith: cannot write text output to stdout")
                .count(),
            1,
            "{case} stderr:\n{stderr}"
        );
        assert!(!stderr.contains("panicked at"), "{case} stderr:\n{stderr}");
    }

    let (reader, writer) = std::io::pipe().expect("creates a pipe");
    drop(reader);
    let closed_both_input = example_asset("clip.glb");
    let status = animsmith()
        .args(["inspect", closed_both_input.to_str().unwrap()])
        .stdout(Stdio::from(writer))
        .stderr(Stdio::null())
        .status()
        .expect("runs inspect with both reporting streams unavailable");
    assert_eq!(
        status.code(),
        Some(0),
        "a closed diagnostic stream must not turn reporting into a panic"
    );
}

#[test]
fn closed_stdout_help_and_version_are_checked_successful_deliveries() {
    for (case, args) in [
        ("root help", vec!["--help"]),
        ("subcommand help", vec!["fix", "--help"]),
        ("version", vec!["--version"]),
    ] {
        let (reader, writer) = std::io::pipe().expect("creates a pipe");
        drop(reader);
        let output = animsmith()
            .args(args)
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawns {case}: {error}"))
            .wait_with_output()
            .unwrap_or_else(|error| panic!("waits for {case}: {error}"));

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{case} remains a successful parser outcome; stderr:\n{stderr}"
        );
        assert_eq!(
            stderr
                .matches("animsmith: cannot write text output to stdout")
                .count(),
            1,
            "{case} stderr:\n{stderr}"
        );
        assert!(!stderr.contains("panicked at"), "{case} stderr:\n{stderr}");
    }
}

#[test]
fn parser_and_json_reporting_survive_both_output_streams_being_closed() {
    let clean = example_asset("clip.glb").display().to_string();
    for (case, args) in [
        ("root help", vec!["--help".to_owned()]),
        ("version", vec!["--version".to_owned()]),
        (
            "JSON measure",
            vec![
                "measure".to_owned(),
                clean,
                "--format".to_owned(),
                "json".to_owned(),
            ],
        ),
    ] {
        let (reader, writer) = std::io::pipe().expect("creates a pipe");
        drop(reader);
        let status = animsmith()
            .args(args)
            .stdout(Stdio::from(writer))
            .stderr(Stdio::null())
            .status()
            .unwrap_or_else(|error| panic!("runs {case} with both streams unavailable: {error}"));
        assert_eq!(
            status.code(),
            Some(0),
            "{case} must not panic through its production stderr wrapper"
        );
    }
}

#[test]
fn forced_color_help_preserves_clap_styling_through_checked_stdout() {
    for (case, args, marker) in [
        ("root", vec!["--help"], "Commands:"),
        ("subcommand", vec!["fix", "--help"], "--repair"),
    ] {
        let output = animsmith()
            .args(args)
            .env_remove("NO_COLOR")
            .env("CLICOLOR_FORCE", "1")
            .output()
            .unwrap_or_else(|error| panic!("runs forced-color {case} help: {error}"));
        assert_eq!(output.status.code(), Some(0), "{case} help");
        assert!(output.stderr.is_empty(), "{case} help stderr");
        assert!(
            output.stdout.windows(2).any(|bytes| bytes == b"\x1b["),
            "forced-color {case} help must retain ANSI styling"
        );
        let visible = String::from_utf8_lossy(&output.stdout);
        assert!(visible.contains("Usage:"), "{case} help output:\n{visible}");
        assert!(visible.contains(marker), "{case} help output:\n{visible}");
    }
}

#[test]
fn forced_color_help_into_closed_stdout_is_diagnosed_without_a_panic() {
    for (case, args) in [
        ("root", vec!["--help"]),
        ("subcommand", vec!["fix", "--help"]),
    ] {
        let (reader, writer) = std::io::pipe().expect("creates a pipe");
        drop(reader);
        let output = animsmith()
            .args(args)
            .env_remove("NO_COLOR")
            .env("CLICOLOR_FORCE", "1")
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawns forced-color {case} help: {error}"))
            .wait_with_output()
            .unwrap_or_else(|error| panic!("waits for forced-color {case} help: {error}"));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{case} stderr:\n{stderr}");
        assert_eq!(
            stderr
                .matches("animsmith: cannot write text output to stdout")
                .count(),
            1,
            "{case} stderr:\n{stderr}"
        );
        assert!(!stderr.contains("panicked at"), "{case} stderr:\n{stderr}");
    }
}

#[test]
fn closed_stdout_fix_with_multiple_reports_is_diagnosed_once() {
    let dir = unique_temp_dir("closed-stdout-fix-multiple");
    let input = dir.path().join("distinct-repairs.glb");
    write_distinct_repair_glb(&input);
    let (reader, writer) = std::io::pipe().expect("creates a pipe");
    drop(reader);
    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 fixture path"),
            "--dry-run",
            "--repair",
            "quat-norm,quat-flip",
        ])
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawns fix")
        .wait_with_output()
        .expect("waits for fix");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "pending repairs remain findings; stderr:\n{stderr}"
    );
    assert_eq!(
        stderr
            .matches("animsmith: cannot write text output to stdout")
            .count(),
        1,
        "one attempted fix stream must produce one diagnostic:\n{stderr}"
    );
    assert!(!stderr.contains("panicked at"), "stderr:\n{stderr}");
}

#[test]
fn closed_stdout_successful_fix_publishes_and_keeps_exit_0() {
    let dir = unique_temp_dir("closed-stdout-fix-published");
    let input = dir.path().join("distinct-repairs.glb");
    let output = dir.path().join("fixed.glb");
    write_distinct_repair_glb(&input);
    let (reader, writer) = std::io::pipe().expect("creates a pipe");
    drop(reader);
    let result = animsmith()
        .arg("fix")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .args(["--repair", "quat-norm,quat-flip"])
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawns fix")
        .wait_with_output()
        .expect("waits for fix");

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(0), "stderr:\n{stderr}");
    assert_eq!(
        stderr
            .matches("animsmith: cannot write text output to stdout")
            .count(),
        1,
        "one published fix transcript must diagnose once:\n{stderr}"
    );
    assert!(
        output.is_file(),
        "the successful fix artifact was published"
    );
    animsmith_gltf::load(&output).expect("the published fixed artifact reloads");
}

// --- #30: exit-code, config-path, and inspect contract ---

fn write_config(dir: &std::path::Path, name: &str, toml: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, toml).expect("writes config");
    path
}

fn example_config() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/character.animsmith.toml")
}

#[test]
fn required_bones_cli_distinguishes_static_presence_missing_ambiguity_and_empty_skeletons() {
    let dir = unique_temp_dir("required-bones");
    let input = dir.path().join("static-rig.glb");
    let empty = dir.path().join("empty.glb");
    write_required_bones_glb(&input);
    write_empty_skeleton_glb(&empty);

    let clean = write_config(
        dir.path(),
        "clean.toml",
        "[rig]\nrequired_bones = [\"root\", \"weapon_socket\"]\n",
    );
    let clean_output = animsmith()
        .arg("--config")
        .arg(&clean)
        .args([
            "lint",
            input.to_str().expect("utf-8 input"),
            "--select",
            "required-bones",
            "--format",
            "json",
        ])
        .output()
        .expect("runs required-bones against static glTF");
    assert!(
        clean_output.status.success(),
        "stderr:\n{}",
        stderr(&clean_output)
    );
    let clean_json: Value = serde_json::from_slice(&clean_output.stdout).expect("valid JSON");
    let clean_check = clean_json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "required-bones")
        .expect("required-bones record");
    assert_eq!(clean_check["applicability"], "applicable");
    assert_eq!(clean_check["evaluation"], "complete");
    assert_eq!(clean_check["findings"], json!([]));
    assert_eq!(
        clean_check["evaluated_scopes"],
        json!([{ "code": "required_bone_presence" }])
    );
    assert_output_schema_valid(&clean_json);

    let failing = write_config(
        dir.path(),
        "failing.toml",
        "[rig]\nrequired_bones = [\"weapon_socket\", \"weapon_socket\", \"missing_socket\", \"duplicate\"]\n",
    );
    let failing_output = animsmith()
        .arg("--config")
        .arg(&failing)
        .args([
            "lint",
            input.to_str().expect("utf-8 input"),
            "--select",
            "required-bones",
            "--format",
            "json",
        ])
        .output()
        .expect("runs required-bones failure cases");
    assert_eq!(
        failing_output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&failing_output)
    );
    let failing_json: Value = serde_json::from_slice(&failing_output.stdout).expect("valid JSON");
    let findings = failing_json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "required-bones")
        .expect("required-bones record")["findings"]
        .as_array()
        .expect("findings");
    assert_eq!(findings.len(), 2, "duplicate declarations must deduplicate");
    assert!(findings.iter().any(|finding| {
        finding["bone"] == "missing_socket"
            && finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("does not exist"))
    }));
    assert!(findings.iter().any(|finding| {
        finding["bone"] == "duplicate"
            && finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("ambiguous"))
    }));
    assert_output_schema_valid(&failing_json);

    let empty_output = animsmith()
        .arg("--config")
        .arg(&clean)
        .args([
            "lint",
            empty.to_str().expect("utf-8 empty input"),
            "--select",
            "required-bones",
            "--format",
            "json",
        ])
        .output()
        .expect("runs required-bones against empty skeleton");
    assert!(
        empty_output.status.success(),
        "stderr:\n{}",
        stderr(&empty_output)
    );
    let empty_json: Value = serde_json::from_slice(&empty_output.stdout).expect("valid JSON");
    let empty_check = empty_json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "required-bones")
        .expect("required-bones record");
    assert_eq!(
        empty_check["evaluation"], "not_evaluated",
        "a skeleton-unavailable gap completes no structural work: {empty_json:#}"
    );
    assert_eq!(empty_check["findings"], json!([]));
    let gaps = empty_check["gaps"].as_array().expect("coverage gaps");
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0]["code"], "skeleton_unavailable");
    assert_eq!(
        gaps[0]["scope"],
        json!({ "code": "required_bone_presence" })
    );
    assert_output_schema_valid(&empty_json);
}

#[cfg(feature = "fbx")]
#[test]
fn required_bones_cli_lints_direct_fbx_input() {
    let dir = unique_temp_dir("required-bones-fbx");
    let config = write_config(
        dir.path(),
        "fbx.toml",
        "[rig]\nrequired_bones = [\"tri\", \"missing_socket\"]\n",
    );
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../animsmith-fbx/testdata/rigged_triangle.fbx");
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            input.to_str().expect("utf-8 FBX fixture"),
            "--select",
            "required-bones",
            "--format",
            "json",
        ])
        .output()
        .expect("lints direct FBX input");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let findings = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "required-bones")
        .expect("required-bones record")["findings"]
        .as_array()
        .expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["bone"], "missing_socket");
    assert_output_schema_valid(&json);
}

#[test]
fn lint_file_with_only_coverage_gaps_exits_zero() {
    let output = animsmith()
        .args(["lint", fixture("rig.gltf").to_str().expect("utf-8 path")])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("coverage[bind-pose]"),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stdout(&output).contains("0 error(s)"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn lint_markdown_renders_findings_for_failing_asset() {
    let dir = unique_temp_dir("markdown-findings");
    let input = dir.path().join("dirty.glb");
    write_distinct_repair_glb(&input); // quat-norm error + quat-flip warning
    let path = input.to_str().expect("utf-8 path");

    let output = animsmith()
        .args(["lint", path, "--format", "markdown"])
        .output()
        .expect("runs animsmith");
    // A failing asset exits 1 in markdown mode just like text/json — the
    // renderer must not swallow the content-failure status.
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);

    // Presentation surface: a heading, the per-clip table header, the
    // collapsible section, and both findings' check ids and severities.
    assert!(out.contains("## animsmith lint"), "stdout:\n{out}");
    assert!(
        out.contains("| Severity | Check | Location | Measured | Expected | Message |"),
        "stdout:\n{out}"
    );
    assert!(out.contains("<details"), "stdout:\n{out}");
    assert!(out.contains("#### clip `sway`"), "stdout:\n{out}");
    assert!(out.contains("`quat-norm`"), "stdout:\n{out}");
    assert!(out.contains("`quat-flip`"), "stdout:\n{out}");
    // End-to-end smoke check that the summary footer reaches stdout;
    // per-branch tallies/grouping/escaping are pinned by the render unit
    // tests in the binary crate. Anchor on the footer's `**N file**`
    // prefix so this matches the aggregate line, not the per-file header.
    assert!(
        out.contains("**1 file** — ❌ 1 error(s) · ⚠️ 1 warning(s)"),
        "stdout:\n{out}"
    );
}

#[test]
fn lint_markdown_escapes_unicode_paragraph_separator_at_the_cli_boundary() {
    let dir = unique_temp_dir("markdown-presentation-controls");
    let input = dir.path().join("hostile.glb");
    let hostile = "forged\nline\u{1b}[31m\u{2028}left\u{2029}right\u{202e}";
    write_hostile_glb(&input, hostile, true);

    let output = animsmith()
        .arg("lint")
        .arg(&input)
        .args(["--format", "markdown"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let markdown = stdout(&output);
    assert!(
        !markdown.contains('\u{2028}') && !markdown.contains('\u{2029}'),
        "raw Unicode separators leaked into Markdown:\n{markdown}"
    );
    assert!(
        !markdown.contains('\u{202e}') && markdown.contains("\\u{202e}"),
        "bidi override was not rendered visibly:\n{markdown}"
    );
    assert!(
        markdown.contains("left right"),
        "the paragraph separator was deleted instead of flattened:\n{markdown}"
    );
}

#[test]
fn lint_markdown_surfaces_nonblocking_coverage_gaps() {
    let dir = unique_temp_dir("markdown-clean");
    let input = dir.path().join("clean.glb");
    write_clean_glb(&input);
    let path = input.to_str().expect("utf-8 path");

    let output = animsmith()
        .args(["lint", path, "--format", "markdown"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let out = stdout(&output);
    assert!(out.contains("0 error(s)"), "stdout:\n{out}");
    assert!(out.contains("coverage gap(s)"), "stdout:\n{out}");
    assert!(
        out.contains("`insufficient_rotation_evidence`"),
        "stdout:\n{out}"
    );
}

#[test]
fn lint_warnings_pass_but_deny_warnings_fails() {
    let dir = unique_temp_dir("deny-warnings");
    let input = dir.path().join("flipped.glb");
    write_flipped_glb(&input); // quat-flip → warning
    let path = input.to_str().expect("utf-8 path");

    // Warnings alone are exit 0.
    let output = animsmith().args(["lint", path]).output().expect("runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("quat-flip"),
        "stdout:\n{}",
        stdout(&output)
    );

    // --deny-warnings promotes the exit to 1.
    let output = animsmith()
        .args(["lint", path, "--deny-warnings"])
        .output()
        .expect("runs");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

/// Declared work on an unresolved rig is a typed, nonblocking coverage gap,
/// never a content finding. `--deny-warnings` does not change that policy.
#[test]
fn lint_unresolved_roles_serialize_as_a_gap_and_exit_zero() {
    let dir = unique_temp_dir("coverage-gap");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input); // root->spine rig: no hips/foot roles resolve
    let config = dir.path().join("animsmith.toml");
    std::fs::write(
        &config,
        "[clips.sway]\nloop = true\n\n[checks.loop-closure]\nseverity = \"off\"\n\n[checks.loop-seam-vel]\nseverity = \"off\"\n",
    )
    .expect("writes config");

    for deny in [false, true] {
        let mut args = vec![
            "--config",
            config.to_str().expect("utf-8 config path"),
            "lint",
            input.to_str().expect("utf-8 input path"),
            "--format",
            "json",
        ];
        if deny {
            args.push("--deny-warnings");
        }
        let output = animsmith().args(&args).output().expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(0),
            "coverage gaps must not fail the run (deny-warnings: {deny}):\n{}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        assert_eq!(json["summary"]["findings"]["note"], 0, "{json:#}");
        let loop_seam = json["files"][0]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["check_id"] == "loop-seam")
            .unwrap();
        assert_eq!(loop_seam["evaluation"], "not_evaluated", "{json:#}");
        assert_eq!(loop_seam["findings"], json!([]), "{json:#}");
        assert_eq!(loop_seam["gaps"][0]["code"], "roles_unresolved", "{json:#}");
    }
}

#[test]
fn lint_text_groups_repeated_per_clip_coverage_gaps() {
    let dir = unique_temp_dir("grouped-coverage-gap");
    let input = dir.path().join("sways.glb");
    write_two_clip_clean_glb(&input);
    let config = dir.path().join("animsmith.toml");
    std::fs::write(&config, "[clips.\"sway*\"]\nloop = true\n").expect("writes config");

    let output = animsmith()
        .args(["--config"])
        .arg(&config)
        .arg("lint")
        .arg(&input)
        .args(["--select", "loop-seam"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let text = stdout(&output);
    assert_eq!(text.matches("coverage[loop-seam]").count(), 1, "{text}");
    assert!(text.contains("roles_unresolved ×2"), "{text}");
    assert!(text.contains("sway, sway_b"), "{text}");
    assert!(text.contains("2 coverage gap(s)"), "{text}");

    let json_output = animsmith()
        .args(["--config"])
        .arg(&config)
        .arg("lint")
        .arg(&input)
        .args(["--select", "loop-seam", "--format", "json"])
        .output()
        .expect("runs JSON lint");
    assert_eq!(
        json_output.status.code(),
        Some(0),
        "{}",
        stderr(&json_output)
    );
    let json: Value = serde_json::from_slice(&json_output.stdout).expect("valid JSON");
    let loop_seam = json["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["check_id"] == "loop-seam")
        .unwrap();
    let subjects: Vec<_> = loop_seam["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|gap| gap["scope"]["subject"].as_str().unwrap())
        .collect();
    assert_eq!(subjects, ["sway", "sway_b"]);
}

#[test]
fn lint_allow_suppresses_a_check() {
    let dir = unique_temp_dir("allow");
    let input = dir.path().join("flipped.glb");
    write_flipped_glb(&input);
    let path = input.to_str().expect("utf-8 path");

    // Positive control: quat-flip fires on this fixture without --allow.
    let baseline = animsmith()
        .args(["lint", path, "--deny-warnings"])
        .output()
        .expect("runs");
    assert_eq!(baseline.status.code(), Some(1), "warning gate baseline");
    assert!(
        stdout(&baseline).contains("quat-flip"),
        "fixture no longer produces quat-flip; suppression test would be vacuous:\n{}",
        stdout(&baseline)
    );

    // With --allow, the same finding is gone.
    let output = animsmith()
        .args(["lint", path, "--allow", "quat-flip", "--deny-warnings"])
        .output()
        .expect("runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        !stdout(&output).contains("quat-flip"),
        "allowed check still reported:\n{}",
        stdout(&output)
    );

    let markdown = animsmith()
        .args([
            "lint",
            path,
            "--format",
            "markdown",
            "--allow",
            "quat-flip",
            "--deny-warnings",
        ])
        .output()
        .expect("runs Markdown renderer");
    assert_eq!(
        markdown.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&markdown)
    );
    assert!(
        !stdout(&markdown).contains("quat-flip"),
        "allowed check still present in Markdown:\n{}",
        stdout(&markdown)
    );
}

#[test]
fn lint_unknown_select_is_operator_error() {
    let missing = "/no/such/selection-precedence.glb";
    let output = animsmith()
        .args(["lint", missing, "--select", "no-such-check"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    let err = stderr(&output);
    assert!(
        err.contains("unknown check 'no-such-check'"),
        "stderr:\n{err}"
    );
    // The error also lists the known check ids so the user can correct
    // the typo without reading the docs.
    assert!(
        err.contains("known:")
            && err.contains("quat-flip")
            && err.contains("engine-addressability"),
        "error should list known check ids:\n{err}"
    );
    assert!(
        !err.contains("failed to read"),
        "selection validation must precede asset I/O:\n{err}"
    );

    let known = animsmith()
        .args(["lint", missing, "--select", "engine-addressability"])
        .output()
        .expect("runs known engine selection");
    assert_eq!(known.status.code(), Some(2), "{}", stderr(&known));
    assert!(
        stderr(&known).contains("failed to read"),
        "{}",
        stderr(&known)
    );
    assert!(
        !stderr(&known).contains("unknown check"),
        "{}",
        stderr(&known)
    );
}

#[test]
fn lint_missing_file_is_operator_error() {
    let output = animsmith()
        .args(["lint", "/no/such/file.glb"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    // Exit 2 is the catch-all; pin that it failed at load (the right
    // cause) rather than arg parsing or config. The loader reads the file
    // itself now, so a missing file is an I/O error, not a parse error.
    // The OS "file not found" text differs across platforms, so anchor on
    // the stable prefix.
    assert!(
        stderr(&output).contains("failed to read"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn lint_bad_config_is_operator_error() {
    let dir = unique_temp_dir("bad-config");
    let config = write_config(dir.path(), "bad.toml", "not valid = = toml [[[\n");
    let output = animsmith()
        .args([
            "--config",
            config.to_str().expect("utf-8 path"),
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("bad config"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn invalid_check_tolerance_is_rejected_before_measurement_input_load() {
    let dir = unique_temp_dir("invalid-check-tolerance");
    let config = write_config(
        dir.path(),
        "invalid-check-tolerance.toml",
        "[checks.loop-seam]\nmin_stride_step_m = nan\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["measure", "/no/such/measurement-input.glb"])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    let error = stderr(&output);
    assert!(error.contains("bad config"), "stderr:\n{error}");
    assert!(error.contains("min_stride_step_m"), "stderr:\n{error}");
    assert!(
        !error.contains("failed to read"),
        "config must fail before measurement input loading:\n{error}"
    );
}

#[test]
fn conflicting_movement_owner_alias_is_rejected_before_measurement_input_load() {
    let dir = unique_temp_dir("conflicting-movement-owner-alias");
    let config = write_config(
        dir.path(),
        "conflicting-movement-owner-alias.toml",
        "[clips.walk]\nmovement_owner_xz = \"gameplay\"\nin_place = true\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["measure", "/no/such/measurement-input.glb"])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    let error = stderr(&output);
    assert!(error.contains("bad config"), "stderr:\n{error}");
    assert!(error.contains("movement_owner_xz"), "stderr:\n{error}");
    assert!(error.contains("in_place"), "stderr:\n{error}");
    assert!(
        !error.contains("failed to read"),
        "config must fail before measurement input loading:\n{error}"
    );
}

#[test]
fn invalid_sync_group_tolerances_are_operator_errors() {
    let dir = unique_temp_dir("invalid-sync-group-tolerance");
    for (name, field, value) in [
        ("negative-duration", "max_duration_delta_s", "-0.001"),
        ("nonfinite-fps", "max_fps_delta", "nan"),
        ("negative-frame-count", "max_frame_count_delta", "-1"),
    ] {
        let config = write_config(
            dir.path(),
            &format!("{name}.toml"),
            &format!(
                "[sync_groups.ring]\nclips = [\"walk\", \"run\"]\nmax_duration_delta_s = {}\nmax_frame_count_delta = {}\nmax_fps_delta = {}\n",
                if field == "max_duration_delta_s" {
                    value
                } else {
                    "0.001"
                },
                if field == "max_frame_count_delta" {
                    value
                } else {
                    "0"
                },
                if field == "max_fps_delta" {
                    value
                } else {
                    "0.01"
                },
            ),
        );
        let output = animsmith()
            .arg("--config")
            .arg(&config)
            .args([
                "lint",
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
                "--select",
                "sync-group",
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(2),
            "{name}: stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        let error = stderr(&output);
        assert!(error.starts_with("animsmith: "), "{name}: {error}");
        assert!(error.contains(field), "{name}: {error}");
    }
}

#[test]
fn invalid_time_complement_settings_are_operator_errors() {
    let dir = unique_temp_dir("invalid-time-complement-setting");
    for (name, field, advantage, amplitude) in [
        (
            "negative-advantage",
            "min_reflected_time_advantage",
            "-0.01",
            "0.03",
        ),
        (
            "advantage-over-one",
            "min_reflected_time_advantage",
            "1.01",
            "0.03",
        ),
        (
            "nonfinite-advantage",
            "min_reflected_time_advantage",
            "nan",
            "0.03",
        ),
        ("negative-amplitude", "min_lr_amplitude_m", "0.25", "-0.01"),
        ("nonfinite-amplitude", "min_lr_amplitude_m", "0.25", "nan"),
    ] {
        let config = write_config(
            dir.path(),
            &format!("{name}.toml"),
            &format!(
                "[sync_groups.ring]\nclips = [\"walk\", \"run\"]\nmax_duration_delta_s = 0.001\nmax_frame_count_delta = 0\nmax_fps_delta = 0.01\n\n[sync_groups.ring.time_complement]\nmin_reflected_time_advantage = {advantage}\nmin_lr_amplitude_m = {amplitude}\n"
            ),
        );
        let output = animsmith()
            .arg("--config")
            .arg(&config)
            .args([
                "lint",
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
                "--select",
                "time-complement",
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(2),
            "{name}: stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        let error = stderr(&output);
        assert!(error.starts_with("animsmith: "), "{name}: {error}");
        assert!(error.contains(field), "{name}: {error}");
    }
}

/// The `--config` TOML path is otherwise only reached through the CLI:
/// a config that disables `quat-flip` must suppress it on a flipped
/// clip, proving `toml::from_str` → `Config` → severity handling works
/// end to end.
#[test]
fn config_toml_path_drives_check_behaviour() {
    let dir = unique_temp_dir("config-toml");
    let input = dir.path().join("flipped.glb");
    write_flipped_glb(&input);
    let path = input.to_str().expect("utf-8 path");
    let config = write_config(
        dir.path(),
        "animsmith.toml",
        "[checks.quat-flip]\nseverity = \"off\"\n",
    );

    // Positive control: without the config, quat-flip fires.
    let baseline = animsmith().args(["lint", path]).output().expect("runs");
    assert!(
        stdout(&baseline).contains("quat-flip"),
        "fixture no longer produces quat-flip; the config test would be vacuous:\n{}",
        stdout(&baseline)
    );

    // The TOML config turns it off end to end.
    let output = animsmith()
        .args([
            "--config",
            config.to_str().expect("utf-8 path"),
            "lint",
            path,
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        !stdout(&output).contains("quat-flip"),
        "off check still reported via TOML config:\n{}",
        stdout(&output)
    );
}

#[test]
fn duration_toml_pins_report_structured_evidence() {
    let dir = unique_temp_dir("duration-pin");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    for (name, text) in [
        (
            "glob.toml",
            "[clips.\"sway*\"]\nduration_s = { value = 1.25, tolerance = 0.125 }\n",
        ),
        (
            "exact.toml",
            "[clips.\"sway*\"]\nduration_s = { value = 1.0, tolerance = 0.0 }\n\
             [clips.sway]\nduration_s = { value = 1.25, tolerance = 0.125 }\n",
        ),
    ] {
        let config = write_config(dir.path(), name, text);
        let output = animsmith()
            .args(["--config"])
            .arg(&config)
            .args(["lint"])
            .arg(&input)
            .args(["--select", "duration-sanity", "--format", "json"])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(1),
            "stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        let duration = json["files"][0]["checks"]
            .as_array()
            .expect("checks array")
            .iter()
            .find(|check| check["check_id"] == "duration-sanity")
            .expect("duration-sanity record");
        let findings = duration["findings"].as_array().expect("findings array");
        assert_eq!(findings.len(), 1, "{json:#}");
        assert_eq!(findings[0]["check_id"], "duration-sanity");
        assert_eq!(findings[0]["severity"], "error");
        assert_eq!(findings[0]["clip"], "sway");
        assert_eq!(findings[0]["measured"], 1.0);
        assert_eq!(findings[0]["expected"], 1.25);
    }
}

#[test]
fn invalid_duration_pin_from_toml_is_an_explicit_error() {
    let dir = unique_temp_dir("invalid-duration-pin");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let config = write_config(
        dir.path(),
        "animsmith.toml",
        "[clips.sway]\nduration_s = { value = nan, tolerance = 0.125 }\n",
    );
    let output = animsmith()
        .args(["--config"])
        .arg(&config)
        .args(["lint"])
        .arg(&input)
        .args(["--select", "duration-sanity", "--format", "json"])
        .output()
        .expect("runs animsmith");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let finding = json["files"][0]["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["check_id"] == "duration-sanity")
        .expect("duration-sanity record")["findings"][0]
        .clone();
    assert_eq!(finding["severity"], "error");
    assert!(
        finding["message"]
            .as_str()
            .expect("message")
            .contains("invalid declared duration pin")
    );
}

#[test]
fn compatible_gltf_profiles_leave_measure_bytes_identical_and_bevy_lint_predicts_the_label() {
    let dir = unique_temp_dir("engine-profile-measure-neutral");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let baseline = animsmith()
        .args(["measure"])
        .arg(&input)
        .args(["--format", "json"])
        .output()
        .expect("runs baseline measure");
    assert_eq!(baseline.status.code(), Some(0), "{}", stderr(&baseline));

    let profiles = [
        (
            "bevy.toml",
            r#"
[engine]
profile = "bevy"
profile_revision = 1
engine_version = "0.19.0"
importer = "gltf-asset-loader"
"#,
        ),
        (
            "godot.toml",
            r#"
[engine]
profile = "godot"
profile_revision = 1
engine_version = "4.7"
importer = "resource-importer-scene"
"#,
        ),
    ];
    for (name, text) in profiles {
        let config = write_config(dir.path(), name, text);
        let profiled = animsmith()
            .args(["--config"])
            .arg(&config)
            .args(["measure"])
            .arg(&input)
            .args(["--format", "json"])
            .output()
            .expect("runs profiled measure");
        assert_eq!(profiled.status.code(), Some(0), "{}", stderr(&profiled));
        assert_eq!(profiled.stdout, baseline.stdout, "profile {name}");
    }

    let changed = dir.path().join("trajectory.glb");
    write_root_trajectory_glb(&changed);
    let baseline_diff = animsmith()
        .args(["diff"])
        .arg(&input)
        .arg(&changed)
        .args(["--format", "json"])
        .output()
        .expect("runs baseline diff");
    assert_eq!(
        baseline_diff.status.code(),
        Some(1),
        "{}",
        stderr(&baseline_diff)
    );
    for (name, _) in profiles {
        let profiled_diff = animsmith()
            .args(["--config"])
            .arg(dir.path().join(name))
            .args(["diff"])
            .arg(&input)
            .arg(&changed)
            .args(["--format", "json"])
            .output()
            .expect("runs profiled diff");
        assert_eq!(
            profiled_diff.status.code(),
            Some(1),
            "{name}: {}",
            stderr(&profiled_diff)
        );
        assert_eq!(profiled_diff.stdout, baseline_diff.stdout, "profile {name}");
    }

    let config = dir.path().join("bevy.toml");
    let baseline_lint = animsmith()
        .args(["lint"])
        .arg(&input)
        .args(["--format", "json"])
        .output()
        .expect("runs baseline lint");
    assert_eq!(
        baseline_lint.status.code(),
        Some(0),
        "{}",
        stderr(&baseline_lint)
    );
    let profiled_lint = animsmith()
        .args(["--config"])
        .arg(&config)
        .args(["lint"])
        .arg(&input)
        .args(["--format", "json"])
        .output()
        .expect("runs profiled lint");
    assert_eq!(
        profiled_lint.status.code(),
        Some(0),
        "{}",
        stderr(&profiled_lint)
    );
    let baseline_json: Value =
        serde_json::from_slice(&baseline_lint.stdout).expect("baseline lint JSON");
    let profiled_json: Value =
        serde_json::from_slice(&profiled_lint.stdout).expect("profiled lint JSON");
    assert!(baseline_json["files"][0]["prediction_provenance"].is_null());
    assert!(profiled_json["files"][0]["prediction_provenance"].is_object());
    assert_eq!(
        profiled_json["summary"]["prediction_facets"],
        json!({
            "available": 1,
            "required_prediction_unavailable": 0
        })
    );
    let baseline_engine = baseline_json["files"][0]["checks"]
        .as_array()
        .expect("baseline checks")
        .iter()
        .find(|check| check["check_id"] == "engine-addressability")
        .expect("baseline engine-addressability check");
    assert_eq!(baseline_engine["applicability"], "not_applicable");
    assert!(baseline_engine.get("prediction").is_none());

    let profiled_engine = profiled_json["files"][0]["checks"]
        .as_array()
        .expect("profiled checks")
        .iter()
        .find(|check| check["check_id"] == "engine-addressability")
        .expect("profiled engine-addressability check");
    assert_eq!(profiled_engine["applicability"], "applicable");
    assert_eq!(profiled_engine["evaluation"], "complete");
    assert_eq!(
        profiled_engine["prediction"]["facets"][0]["scope"],
        json!({
            "code": "animation_asset_label",
            "subject": "Animation0"
        })
    );
    assert_eq!(
        profiled_engine["prediction"]["provenance_identity"],
        profiled_json["files"][0]["prediction_provenance"]["identity"]
    );
    assert_eq!(profiled_engine["findings"], json!([]));

    let core_checks = |json: &Value| {
        json["files"][0]["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .filter(|check| check["check_id"] != "engine-addressability")
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(core_checks(&profiled_json), core_checks(&baseline_json));
    assert_eq!(
        profiled_json["files"][0]["measurements"],
        baseline_json["files"][0]["measurements"]
    );
}

#[test]
fn bevy_v3_track_support_resolves_before_io_and_observes_gate_outcomes() {
    let dir = unique_temp_dir("bevy-v3-track-gates");
    let missing = dir.path().join("missing.gltf");
    let valid = write_bevy_v3_track_config(dir.path(), "valid", false, None);
    let output = animsmith()
        .arg("--config")
        .arg(&valid)
        .args(["lint", "--select", "engine-track-support"])
        .arg(&missing)
        .output()
        .expect("runs valid revision-3 configuration before input IO");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("failed to read"),
        "{}",
        stderr(&output)
    );

    let missing_feature = write_config(
        dir.path(),
        "bevy-v3-missing-feature.toml",
        r#"
[engine]
profile = "bevy"
profile_revision = 3
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
"#,
    );
    let output = animsmith()
        .arg("--config")
        .arg(missing_feature)
        .args(["lint", "--select", "engine-track-support"])
        .arg(&missing)
        .output()
        .expect("rejects incomplete revision-3 configuration before input IO");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("bevy_animation_feature"),
        "{}",
        stderr(&output)
    );

    let source = dir.path().join("one-animation-one-channel.gltf");
    write_track_support_gltf(&source, &[1]);
    let run = |name: &str, feature: bool, load: Option<bool>| {
        let config = write_bevy_v3_track_config(dir.path(), name, feature, load);
        let output = animsmith()
            .arg("--config")
            .arg(config)
            .args([
                "lint",
                "--select",
                "engine-track-support",
                "--format",
                "json",
            ])
            .arg(&source)
            .output()
            .expect("runs revision-3 track support lint");
        assert!(
            output.status.success() || output.status.code() == Some(1),
            "stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("track support JSON");
        (output, json)
    };

    let (disabled, disabled_json) = run("feature-disabled", false, None);
    assert_eq!(disabled.status.code(), Some(0), "{}", stderr(&disabled));
    assert_output_schema_valid(&disabled_json);
    let disabled_check = lint_check(&disabled_json, "engine-track-support");
    assert_eq!(disabled_check["findings"], json!([]));
    assert_eq!(
        track_support_facets(&disabled_json)
            .iter()
            .map(|facet| facet["scope"]["subject"].as_str().expect("subject"))
            .collect::<Vec<_>>(),
        vec!["source_animation:0", "source_animation:0:source_channel:0"]
    );
    for (index, facet) in track_support_facets(&disabled_json).iter().enumerate() {
        assert_eq!(facet["state"], "available");
        assert_eq!(facet["result"]["kind"], "source_import_disposition");
        assert_eq!(facet["result"]["result"]["disposition"], "dropped");
        assert_eq!(
            facet["result"]["result"]["controlling_gate"],
            "bevy_animation_feature"
        );
        let row_field = if index == 0 {
            "raw_animation_channel_inventory.animation_row"
        } else {
            "raw_animation_channel_inventory.channel_row"
        };
        assert!(
            facet["basis"]["references"]
                .as_array()
                .expect("basis references")
                .iter()
                .any(|reference| reference.to_string().contains(row_field)),
            "exact source row {row_field} is retained in {facet:#}"
        );
    }
    let settings = &disabled_json["files"][0]["prediction_provenance"]["base"]["settings"]["document_settings"];
    assert!(
        settings
            .as_array()
            .expect("settings")
            .iter()
            .any(|setting| setting.to_string().contains("load_animations")
                && setting.to_string().contains("profile_default")),
        "default-origin load setting: {settings:#}"
    );

    let (both_disabled, both_disabled_json) = run("both-disabled", false, Some(false));
    assert_eq!(
        both_disabled.status.code(),
        Some(0),
        "{}",
        stderr(&both_disabled)
    );
    assert!(
        track_support_facets(&both_disabled_json)
            .iter()
            .all(|facet| {
                facet["result"]["result"]["controlling_gate"] == "bevy_animation_feature"
            })
    );

    let (load_disabled, load_disabled_json) = run("load-disabled", true, Some(false));
    assert_eq!(
        load_disabled.status.code(),
        Some(0),
        "{}",
        stderr(&load_disabled)
    );
    assert!(
        track_support_facets(&load_disabled_json)
            .iter()
            .all(|facet| { facet["result"]["result"]["controlling_gate"] == "load_animations" })
    );

    let (positive_gates, positive_gates_json) = run("positive-gates", true, Some(true));
    assert_eq!(
        positive_gates.status.code(),
        Some(1),
        "{}",
        stderr(&positive_gates)
    );
    assert_eq!(
        lint_check(&positive_gates_json, "engine-track-support")["findings"],
        json!([])
    );
    assert!(
        track_support_facets(&positive_gates_json)
            .iter()
            .all(|facet| {
                facet["state"] == "required_prediction_unavailable"
                    && facet["reasons"] == json!(["runtime_animation_survival_unavailable"])
            })
    );
    let output = animsmith()
        .arg("--config")
        .arg(write_bevy_v3_track_config(
            dir.path(),
            "positive-gates-allow",
            true,
            Some(true),
        ))
        .args([
            "lint",
            "--select",
            "engine-track-support",
            "--allow",
            "engine-track-support",
        ])
        .arg(&source)
        .output()
        .expect("runs required-unavailable track support lint with allow");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));

    let empty = dir.path().join("empty.gltf");
    write_track_support_gltf(&empty, &[]);
    let output = animsmith()
        .arg("--config")
        .arg(write_bevy_v3_track_config(dir.path(), "empty", false, None))
        .args([
            "lint",
            "--select",
            "engine-track-support",
            "--format",
            "json",
        ])
        .arg(empty)
        .output()
        .expect("runs complete-empty track support lint");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let empty_json: Value = serde_json::from_slice(&output.stdout).expect("empty JSON");
    let empty_check = lint_check(&empty_json, "engine-track-support");
    assert_eq!(empty_check["applicability"], "not_applicable");
    assert!(empty_check.get("prediction").is_none());
}

#[test]
fn bevy_v3_track_support_saturation_has_canonical_prefix_and_one_summary() {
    let dir = unique_temp_dir("bevy-v3-track-saturation");
    let source = dir.path().join("saturated.gltf");
    write_track_support_gltf(&source, &[4_096]);
    let output = animsmith()
        .arg("--config")
        .arg(write_bevy_v3_track_config(
            dir.path(),
            "saturated",
            false,
            Some(true),
        ))
        .args([
            "lint",
            "--select",
            "engine-track-support",
            "--format",
            "json",
        ])
        .arg(source)
        .output()
        .expect("runs saturated track support lint");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("saturated JSON");
    let facets = track_support_facets(&json);
    assert_eq!(facets.len(), 4_096);
    assert_eq!(facets[0]["scope"]["subject"], "source_animation:0");
    assert_eq!(
        facets[4_095]["scope"]["code"],
        "engine-track-support:facet-budget"
    );
    assert_eq!(facets[4_095]["reasons"], json!(["facet_budget_exceeded"]));
    let subjects = facets[..4_095]
        .iter()
        .map(|facet| {
            facet["scope"]["subject"]
                .as_str()
                .expect("candidate subject")
        })
        .collect::<Vec<_>>();
    let mut canonical = subjects.clone();
    canonical.sort_unstable();
    assert_eq!(
        subjects, canonical,
        "facets retain V4's canonical scope order"
    );
    assert!(subjects.contains(&"source_animation:0:source_channel:4093"));
}

#[test]
fn bevy_v3_track_support_partial_or_unavailable_saturated_inventory_readbacks() {
    let dir = unique_temp_dir("bevy-v3-track-partial-saturated");
    let source_path = dir.path().join("saturated.gltf");
    write_track_support_gltf(&source_path, &[4_096]);
    let config = write_bevy_v3_track_config(dir.path(), "partial-saturated", false, Some(true));
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            "--select",
            "engine-track-support",
            "--format",
            "json",
        ])
        .arg(&source_path)
        .output()
        .expect("produces a saturated complete track report");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let saturated: Value = serde_json::from_slice(&output.stdout).expect("saturated JSON");
    let original: animsmith_core::PredictionProvenanceV5 =
        serde_json::from_value(saturated["files"][0]["prediction_provenance"].clone())
            .expect("V5 provenance decodes");
    let source = animsmith_gltf::load_source(&source_path).expect("source reloads");
    let roles = animsmith_core::ResolvedRoles::default();
    let rig = animsmith_core::RigInfo::from_resolved(source.document(), &roles)
        .expect("empty roles match fixture");
    let measurements = animsmith_core::MeasurementContract::new(
        BTreeMap::new(),
        animsmith_core::measure::measure_assets(source.document()),
    )
    .expect("fixture measurement contract");

    for (name, coverage) in [
        (
            "partial",
            json!({ "state": "partial", "reason": "projection_budget_exceeded" }),
        ),
        (
            "unavailable",
            json!({ "state": "unavailable", "reason": "parser_unavailable" }),
        ),
    ] {
        let mut inventory: Value =
            serde_json::to_value(original.raw_animation_channels()).expect("inventory serializes");
        inventory["animation_coverage"] = coverage;
        inventory["source_coverage_complete"] = json!(false);
        let inventory: animsmith_core::RawAnimationChannelInventoryV1 =
            serde_json::from_value(inventory).expect("partial saturated inventory is valid");
        assert!(
            inventory.candidate_overflow(),
            "{name} keeps the N+1 sentinel"
        );
        assert!(
            !inventory.source_coverage_complete(),
            "{name} coverage is incomplete"
        );
        let provenance =
            animsmith_core::PredictionProvenanceV5::new(original.base().clone(), inventory)
                .expect("successor provenance binds incomplete saturated inventory");
        let v1 = |reference| {
            animsmith_core::PredictionBasisReferenceV4::v2(
                animsmith_core::PredictionBasisReferenceV2::v1(reference),
            )
        };
        let basis = animsmith_core::EnginePredictionBasisV4::new(vec![
            v1(animsmith_core::PredictionBasisReferenceV1::profile_fact(
                "source_import_disposition",
            )
            .expect("profile fact reference")),
            v1(animsmith_core::PredictionBasisReferenceV1::primary_source(
                "bevy-gltf-loader-0.19.0-c6f634ca",
            )
            .expect("loader source reference")),
            v1(animsmith_core::PredictionBasisReferenceV1::primary_source(
                "bevy-feature-manifest-0.19.0-c6f634ca",
            )
            .expect("feature source reference")),
            v1(
                animsmith_core::PredictionBasisReferenceV1::resolved_setting(
                    animsmith_core::ResolvedSettingLocationV1::Document,
                    "bevy_animation_feature",
                )
                .expect("feature setting reference"),
            ),
            v1(
                animsmith_core::PredictionBasisReferenceV1::resolved_setting(
                    animsmith_core::ResolvedSettingLocationV1::Document,
                    "load_animations",
                )
                .expect("load setting reference"),
            ),
            v1(animsmith_core::PredictionBasisReferenceV1::project_field(
                "raw_animation_channel_inventory.animation_coverage",
                animsmith_core::PredictionScalarV1::text(name).expect("coverage state token"),
            )
            .expect("coverage basis reference")),
            v1(animsmith_core::PredictionBasisReferenceV1::project_field(
                "raw_animation_channel_inventory.source_coverage_complete",
                animsmith_core::PredictionScalarV1::Boolean { value: false },
            )
            .expect("aggregate coverage basis reference")),
        ])
        .expect("inventory basis");
        let facet = animsmith_core::EnginePredictionFacetV4::required_unavailable(
            animsmith_core::EvaluationScope::new(animsmith_core::EvaluationScopeCode::custom(
                "engine-track-support:inventory",
            )),
            basis,
            vec![animsmith_core::PredictionUnavailableReasonV2::RawSourceIncomplete],
        )
        .expect("one inventory facet");
        let inner = animsmith_core::EnginePredictionV4::new(
            provenance.base().identity().clone(),
            vec![facet],
        )
        .expect("canonical inventory prediction");
        let prediction = animsmith_core::EnginePredictionV5::new(&provenance, inner)
            .expect("V5 inventory prediction");
        let check = animsmith_core::CheckEvaluation::evaluated(
            "engine-track-support",
            animsmith_core::CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
                .with_engine_prediction_v5(prediction),
        )
        .expect("required-unavailable check lifecycle");
        let file = animsmith_core::LintFileReportV16::new_v5(
            source_path.display().to_string(),
            source.source_facts().primary_identity().clone(),
            rig.clone(),
            Some(provenance),
            vec![check],
            measurements.clone(),
        )
        .expect("V16 producer accepts incomplete saturated inventory without budget summary");
        let report = serde_json::to_value(
            animsmith_core::LintEnvelopeV16::new(
                animsmith_core::ToolInfo::animsmith(animsmith_core::ToolSource::new(None, None)),
                vec![file],
            )
            .expect("V16 envelope"),
        )
        .expect("report serializes");
        assert_output_v16_schema_valid(&report);
        assert_eq!(track_support_facets(&report).len(), 1, "{name}");
        let path = dir.path().join(format!("{name}-saturated.json"));
        write_json(&path, &report);
        let readback = animsmith()
            .arg("diff")
            .arg(&path)
            .arg(&path)
            .output()
            .expect("strictly reads incomplete saturated report");
        assert_eq!(
            readback.status.code(),
            Some(0),
            "{name}: {}",
            stderr(&readback)
        );
    }
}

#[test]
fn bevy_v3_track_support_readback_rejects_hostile_sidecars_and_oversized_inventory() {
    let dir = unique_temp_dir("bevy-v3-track-readback");
    let source = dir.path().join("one-animation-one-channel.gltf");
    write_track_support_gltf(&source, &[1]);
    let output = animsmith()
        .arg("--config")
        .arg(write_bevy_v3_track_config(
            dir.path(),
            "readback",
            false,
            Some(true),
        ))
        .args([
            "lint",
            "--select",
            "engine-track-support",
            "--format",
            "json",
        ])
        .arg(&source)
        .output()
        .expect("produces valid revision-3 track report");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let valid: Value = serde_json::from_slice(&output.stdout).expect("valid track report JSON");
    assert_output_schema_valid(&valid);

    let run_diff = |name: &str, report: &Value| {
        let path = dir.path().join(name);
        write_json(&path, report);
        animsmith()
            .arg("diff")
            .arg(&path)
            .arg(&path)
            .output()
            .expect("strictly reads track sidecar")
    };
    let reject = |name: &str, report: &Value| {
        let output = run_diff(name, report);
        assert_eq!(output.status.code(), Some(2), "{name}: {}", stderr(&output));
        assert!(stdout(&output).is_empty(), "{name}: {}", stdout(&output));
    };

    let mut omitted = valid.clone();
    let facets =
        lint_check_mut(&mut omitted, "engine-track-support")["prediction"]["prediction"]["facets"]
            .as_array_mut()
            .expect("facets");
    facets.pop();
    omitted["summary"]["prediction_facets"]["available"] = json!(1);
    reject("omitted-track-facet.json", &omitted);

    let mut swapped = valid.clone();
    track_support_facets(&swapped);
    lint_check_mut(&mut swapped, "engine-track-support")["prediction"]["prediction"]["facets"]
        .as_array_mut()
        .expect("facets")
        .swap(0, 1);
    reject("swapped-track-facets.json", &swapped);

    let mut forged_scope = valid.clone();
    lint_check_mut(&mut forged_scope, "engine-track-support")["prediction"]["prediction"]["facets"]
        [0]["scope"]["subject"] = json!("source_animation:7");
    reject("forged-track-scope.json", &forged_scope);

    let mut forged_result = valid.clone();
    lint_check_mut(&mut forged_result, "engine-track-support")["prediction"]["prediction"]["facets"]
        [0]["result"]["result"]["disposition"] = json!("preserved");
    reject("forged-track-result.json", &forged_result);

    let mut forged_gate = valid.clone();
    lint_check_mut(&mut forged_gate, "engine-track-support")["prediction"]["prediction"]["facets"]
        [0]["result"]["result"]["controlling_gate"] = json!("load_animations");
    reject("forged-track-gate.json", &forged_gate);

    let mut forged_finding = valid.clone();
    lint_check_mut(&mut forged_finding, "engine-track-support")["findings"] = json!([{
        "check_id": "engine-track-support",
        "severity": "note",
        "message": "forged finding"
    }]);
    forged_finding["summary"]["findings"]["note"] = json!(1);
    reject("forged-track-finding.json", &forged_finding);

    let mut forged_profile = valid.clone();
    forged_profile["files"][0]["prediction_provenance"]["base"]["profile"]["identity"]["sha256"] =
        json!("0".repeat(64));
    reject("forged-track-profile-identity.json", &forged_profile);

    let mut forged_coverage = valid.clone();
    forged_coverage["files"][0]["prediction_provenance"]["raw_animation_channels"]["source_coverage_complete"] =
        json!(false);
    reject("forged-track-coverage.json", &forged_coverage);

    let mut forged_indices = valid.clone();
    forged_indices["files"][0]["prediction_provenance"]["raw_animation_channels"]["rows"][1]["source_channel_index"] =
        json!(1);
    reject("forged-track-indices.json", &forged_indices);

    let mut forged_contracts = valid.clone();
    forged_contracts["files"][0]["prediction_provenance"]["consumed_contracts"][0] =
        json!("urn:animsmith:forged-contract:1");
    reject("forged-track-consumed-contracts.json", &forged_contracts);

    let mut oversized = valid;
    let rows = oversized["files"][0]["prediction_provenance"]["raw_animation_channels"]["rows"]
        .as_array()
        .expect("rows")
        .clone();
    oversized["files"][0]["prediction_provenance"]["raw_animation_channels"]["rows"] =
        Value::Array((0..4_098).flat_map(|_| rows.clone()).collect());
    reject("oversized-track-inventory.json", &oversized);
}

#[test]
fn bevy_animation_labels_are_indexed_independently_of_source_names_and_share_one_lifecycle() {
    let dir = unique_temp_dir("bevy-animation-labels");
    let input = dir.path().join("animations.gltf");
    write_source_animation_inventory_gltf(&input, &[Some("duplicate"), None, Some("duplicate")]);
    let config = write_bevy_config(dir.path(), "labels");

    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            "--select",
            "engine-addressability",
            "--format",
            "json",
        ])
        .arg(&input)
        .output()
        .expect("runs Bevy addressability lint");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid lint JSON");
    assert_output_schema_valid(&json);
    let check = lint_check(&json, "engine-addressability");
    assert_eq!(check["selection"], "selected");
    assert_eq!(check["configuration"], "enabled");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "complete");
    assert_eq!(check["findings"], json!([]));
    let facets = check["prediction"]["facets"]
        .as_array()
        .expect("prediction facets");
    assert_eq!(facets.len(), 3);
    assert_eq!(
        facets
            .iter()
            .map(|facet| facet["scope"]["subject"].as_str().expect("label subject"))
            .collect::<Vec<_>>(),
        vec!["Animation0", "Animation1", "Animation2"]
    );
    for (index, facet) in facets.iter().enumerate() {
        assert_eq!(facet["scope"]["code"], "animation_asset_label");
        assert_eq!(facet["state"], "available");
        assert_eq!(facet["reasons"], json!([]));
        assert_eq!(
            facet["basis"]["references"],
            json!([
                {
                    "contract": "v1",
                    "reference": {
                        "kind": "raw_source",
                        "domain": "clip",
                        "key": {
                            "kind": "clip",
                            "source_clip_index": index
                        },
                        "field": "source_name.state",
                        "value": {
                            "type": "token",
                            "value": if index == 1 { "proven_absent" } else { "observed" }
                        }
                    }
                },
                {
                    "contract": "v1",
                    "reference": {
                        "kind": "profile_fact",
                        "fact_id": "animation_addressability"
                    }
                },
                {
                    "contract": "v1",
                    "reference": {
                        "kind": "primary_source",
                        "source_id": "bevy-gltf-asset-label-0.19.0"
                    }
                }
            ])
        );
    }
    assert_eq!(
        check["prediction"]["provenance_identity"],
        json["files"][0]["prediction_provenance"]["identity"]
    );
    assert_eq!(
        json["summary"]["prediction_facets"],
        json!({
            "available": 3,
            "required_prediction_unavailable": 0
        })
    );

    let unselected = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["lint", "--select", "nan", "--format", "json"])
        .arg(&input)
        .output()
        .expect("runs unselected Bevy addressability lint");
    assert_eq!(unselected.status.code(), Some(0), "{}", stderr(&unselected));
    let unselected: Value = serde_json::from_slice(&unselected.stdout).expect("valid lint JSON");
    let check = lint_check(&unselected, "engine-addressability");
    assert_eq!(check["selection"], "unselected");
    assert_eq!(check["configuration"], "enabled");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "not_evaluated");
    assert!(check.get("prediction").is_none());

    let disabled_config = write_config(
        dir.path(),
        "bevy-disabled.toml",
        r#"
[engine]
profile = "bevy"
profile_revision = 1
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[checks.engine-addressability]
severity = "off"
"#,
    );
    let disabled = animsmith()
        .arg("--config")
        .arg(&disabled_config)
        .args([
            "lint",
            "--select",
            "engine-addressability",
            "--format",
            "json",
        ])
        .arg(&input)
        .output()
        .expect("runs disabled Bevy addressability lint");
    assert_eq!(disabled.status.code(), Some(0), "{}", stderr(&disabled));
    let disabled: Value = serde_json::from_slice(&disabled.stdout).expect("valid lint JSON");
    let check = lint_check(&disabled, "engine-addressability");
    assert_eq!(check["selection"], "selected");
    assert_eq!(check["configuration"], "disabled");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "not_evaluated");
    assert!(check.get("prediction").is_none());

    for (format, needle) in [
        (
            "text",
            "animation_asset_label subject Animation0: available",
        ),
        (
            "markdown",
            "| `engine-addressability` | `animation_asset_label` | `Animation0` | available |",
        ),
    ] {
        let rendered = animsmith()
            .arg("--config")
            .arg(&config)
            .args([
                "lint",
                "--select",
                "engine-addressability",
                "--format",
                format,
            ])
            .arg(&input)
            .output()
            .expect("renders Bevy addressability lint");
        assert_eq!(rendered.status.code(), Some(0), "{}", stderr(&rendered));
        assert!(stdout(&rendered).contains(needle), "{}", stdout(&rendered));
    }
}

#[test]
fn bevy_complete_empty_and_absent_profile_records_are_not_applicable() {
    let dir = unique_temp_dir("bevy-empty-animation-labels");
    let input = dir.path().join("empty.gltf");
    write_source_animation_inventory_gltf(&input, &[]);
    let config = write_bevy_config(dir.path(), "empty");

    for config in [None, Some(config.as_path())] {
        let mut command = animsmith();
        if let Some(config) = config {
            command.arg("--config").arg(config);
        }
        let output = command
            .args([
                "lint",
                "--select",
                "engine-addressability",
                "--format",
                "json",
            ])
            .arg(&input)
            .output()
            .expect("runs empty addressability lint");
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid lint JSON");
        let check = lint_check(&json, "engine-addressability");
        assert_eq!(check["applicability"], "not_applicable");
        assert_eq!(check["evaluation"], "not_evaluated");
        assert!(check.get("prediction").is_none());
    }
}

#[test]
fn bevy_actual_clip_inventory_above_v1_provenance_bound_is_structured_v3_evidence() {
    let dir = unique_temp_dir("bevy-over-limit-animation-labels");
    let input = dir.path().join("over-limit.gltf");
    let names = vec![None; animsmith_core::RAW_SOURCE_V1_MAX_CLIPS + 1];
    write_source_animation_inventory_gltf(&input, &names);
    let config = write_bevy_config(dir.path(), "over-limit");

    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            "--select",
            "engine-addressability",
            "--format",
            "json",
        ])
        .arg(&input)
        .output()
        .expect("runs over-limit Bevy addressability lint");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("structured lint JSON");
    let provenance = &json["files"][0]["prediction_provenance"];
    assert_eq!(provenance["settings"]["clip_coverage"]["state"], "partial");
    assert_eq!(
        provenance["settings"]["clip_coverage"]["reason"],
        "actual_clip_rows_exceeded"
    );
    let check = lint_check(&json, "engine-addressability");
    assert_eq!(
        check["prediction"]["schema"],
        "urn:animsmith:engine-prediction:3"
    );
    assert_eq!(
        check["prediction"]["facets"][0]["reasons"],
        json!(["raw_source_incomplete", "resolved_settings_overflow"])
    );
}

#[test]
fn generate_addressability_actual_clip_inventory_above_v1_bound_is_an_operator_error() {
    let dir = unique_temp_dir("generate-addressability-over-limit");
    let input = dir.path().join("over-limit.gltf");
    let names = vec![None; animsmith_core::RAW_SOURCE_V1_MAX_CLIPS + 1];
    write_source_animation_inventory_gltf(&input, &names);
    let config = write_bevy_config(dir.path(), "generate-over-limit");

    let output = animsmith()
        .args(["generate", "addressability"])
        .arg(&input)
        .output()
        .expect("runs over-limit neutral addressability generation");
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(
        stderr(&output).contains("4097 animations") && stderr(&output).contains("V1 limit of 4096"),
        "{}",
        stderr(&output)
    );

    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["generate", "addressability"])
        .arg(&input)
        .output()
        .expect("runs over-limit Bevy addressability generation");
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(
        stderr(&output).contains("settings.clips")
            && stderr(&output).contains("4097")
            && stderr(&output).contains("4096"),
        "{}",
        stderr(&output)
    );
}

#[cfg(feature = "report")]
#[test]
fn report_runs_the_production_bevy_addressability_check() {
    let dir = unique_temp_dir("bevy-animation-label-report");
    let input = dir.path().join("animations.gltf");
    let report = dir.path().join("report.html");
    write_source_animation_inventory_gltf(&input, &[Some("walk")]);
    let config = write_bevy_config(dir.path(), "report");

    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["report"])
        .arg(&input)
        .args(["--output"])
        .arg(&report)
        .args(["--clip", "missing-normalized-clip"])
        .output()
        .expect("renders Bevy addressability report");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let html = std::fs::read_to_string(&report).expect("reads HTML report");
    assert!(html.contains("engine-addressability"));
    assert!(html.contains("Animation0"));
    assert!(html.contains("animation_asset_label"));
}

#[cfg(feature = "fbx")]
#[test]
fn unreal_fbx_clip_boundary_is_current_v3_fail_closed_and_obeys_lifecycle() {
    let dir = unique_temp_dir("unreal-fbx-clip-boundary");
    let input = dir.path().join("rigged-triangle.fbx");
    std::fs::write(&input, RIGGED_TRIANGLE_FBX).unwrap();
    let config = write_config(
        dir.path(),
        "unreal.toml",
        r#"
[engine]
profile = "unreal"
profile_revision = 1
engine_version = "5.8"
importer = "fbx-importer"
"#,
    );

    let selected = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            "--select",
            "engine-clip-boundary",
            "--format",
            "json",
        ])
        .arg(&input)
        .output()
        .expect("runs exact Unreal boundary lint");
    assert_eq!(selected.status.code(), Some(1), "{}", stderr(&selected));
    let selected: Value = serde_json::from_slice(&selected.stdout).expect("current lint JSON");
    assert_output_schema_valid(&selected);
    assert_eq!(selected["schema"], CURRENT_OUTPUT_SCHEMA_ID);
    assert_eq!(
        selected["files"][0]["prediction_provenance"]["schema"],
        "urn:animsmith:prediction-provenance:3"
    );
    assert_eq!(
        selected["files"][0]["prediction_provenance"]["raw_source"]["schema"],
        animsmith_core::RAW_SOURCE_FACTS_V2_ID
    );
    assert!(
        selected["files"][0]["prediction_provenance"]["raw_source"]["exact_source_timing"]
            .is_object()
    );
    let check = lint_check(&selected, "engine-clip-boundary");
    assert_eq!(check["selection"], "selected");
    assert_eq!(check["configuration"], "enabled");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "not_evaluated");
    assert_eq!(
        check["prediction"]["schema"],
        "urn:animsmith:engine-prediction:3"
    );
    assert_eq!(
        check["prediction"]["facets"][0]["state"],
        "required_prediction_unavailable"
    );
    assert_eq!(
        check["prediction"]["facets"][0]["reasons"],
        json!(["animsmith:source_declared_time_mode_unavailable"])
    );

    let unselected = animsmith()
        .arg("--config")
        .arg(&config)
        .args(["lint", "--select", "nan", "--format", "json"])
        .arg(&input)
        .output()
        .expect("runs unselected boundary lint");
    assert_eq!(unselected.status.code(), Some(0), "{}", stderr(&unselected));
    let unselected: Value = serde_json::from_slice(&unselected.stdout).expect("lint JSON");
    let check = lint_check(&unselected, "engine-clip-boundary");
    assert_eq!(check["selection"], "unselected");
    assert_eq!(check["evaluation"], "not_evaluated");
    assert!(check.get("prediction").is_none());

    let disabled_config = write_config(
        dir.path(),
        "unreal-disabled.toml",
        r#"
[engine]
profile = "unreal"
profile_revision = 1
engine_version = "5.8"
importer = "fbx-importer"

[checks.engine-clip-boundary]
severity = "off"
"#,
    );
    let disabled = animsmith()
        .arg("--config")
        .arg(disabled_config)
        .args([
            "lint",
            "--select",
            "engine-clip-boundary",
            "--format",
            "json",
        ])
        .arg(&input)
        .output()
        .expect("runs disabled boundary lint");
    assert_eq!(disabled.status.code(), Some(0), "{}", stderr(&disabled));
    let disabled: Value = serde_json::from_slice(&disabled.stdout).expect("lint JSON");
    let check = lint_check(&disabled, "engine-clip-boundary");
    assert_eq!(check["configuration"], "disabled");
    assert_eq!(check["evaluation"], "not_evaluated");
    assert!(check.get("prediction").is_none());
}

#[cfg(feature = "fbx")]
#[test]
fn compatible_fbx_profiles_leave_measure_output_byte_identical() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../animsmith-fbx/testdata/rigged_triangle.fbx");
    let baseline = animsmith()
        .args(["measure"])
        .arg(&input)
        .args(["--format", "json"])
        .output()
        .expect("runs baseline FBX measure");
    assert_eq!(baseline.status.code(), Some(0), "{}", stderr(&baseline));

    let dir = unique_temp_dir("engine-profile-fbx-measure-neutral");
    let profiles = [
        (
            "unity-generic.toml",
            r#"
[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
convert_units = true
bake_axis_conversion = true
root_motion_source = "Reference/Root"

[clips."*".engine_settings]
root_rotation = "extract"
root_position_y = "bake"
root_position_xz = "extract"
"#,
        ),
        (
            "unity-humanoid.toml",
            r#"
[engine]
profile = "unity-humanoid"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
convert_units = true
bake_axis_conversion = true

[clips."*".engine_settings]
root_rotation = "extract"
root_position_y = "bake"
root_position_xz = "extract"
"#,
        ),
        (
            "unreal.toml",
            r#"
[engine]
profile = "unreal"
profile_revision = 1
engine_version = "5.8"
importer = "fbx-importer"
"#,
        ),
    ];
    for (name, text) in profiles {
        let config = write_config(dir.path(), name, text);
        let profiled = animsmith()
            .args(["--config"])
            .arg(&config)
            .args(["measure"])
            .arg(&input)
            .args(["--format", "json"])
            .output()
            .expect("runs profiled FBX measure");
        assert_eq!(
            profiled.status.code(),
            Some(0),
            "{name}: {}",
            stderr(&profiled)
        );
        assert_eq!(profiled.stdout, baseline.stdout, "profile {name}");
    }
}

#[test]
fn engine_static_configuration_errors_precede_missing_input_io() {
    let dir = unique_temp_dir("engine-profile-static-errors");
    let missing = dir.path().join("missing.glb");
    let cases = [
        (
            "unknown.toml",
            r#"
[engine]
profile = "bevy-latest"
profile_revision = 1
engine_version = "0.19.0"
importer = "gltf-asset-loader"
"#,
            "unknown engine profile selection",
        ),
        (
            "settings-without-selection.toml",
            r#"
[clips.walk.engine_settings]
root_rotation = "bake"
"#,
            "engine settings were declared without an engine profile selection",
        ),
        (
            "humanoid-root-source.toml",
            r#"
[engine]
profile = "unity-humanoid"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
convert_units = true
bake_axis_conversion = true
root_motion_source = "Reference/Root"
"#,
            "root_motion_source is not applicable",
        ),
        (
            "source-unit.toml",
            "source_unit = \"metre\"\n",
            "unknown field `source_unit`",
        ),
        (
            "engine-source-unit.toml",
            r#"
[engine]
profile = "bevy"
profile_revision = 1
engine_version = "0.19.0"
importer = "gltf-asset-loader"
source_unit = "metre"
"#,
            "unknown field `source_unit`",
        ),
        (
            "wrong-scope.toml",
            r#"
[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
convert_units = true
bake_axis_conversion = true
root_motion_source = "Reference/Root"
root_rotation = "bake"
"#,
            "root_rotation has Clip scope but was declared in Document scope",
        ),
        (
            "wrong-domain.toml",
            r#"
[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
convert_units = "bake"
bake_axis_conversion = true
root_motion_source = "Reference/Root"
"#,
            "invalid value for engine setting convert_units",
        ),
        (
            "missing-required.toml",
            r#"
[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
bake_axis_conversion = true
root_motion_source = "Reference/Root"
"#,
            "missing required engine setting convert_units",
        ),
        (
            "enum-spelling-is-unknown.toml",
            r#"
[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
BakeAxisConversion = true
"#,
            "unknown engine setting \"BakeAxisConversion\"",
        ),
    ];
    for (name, text, expected) in cases {
        let config = write_config(dir.path(), name, text);
        let output = animsmith()
            .args(["--config"])
            .arg(&config)
            .args(["measure"])
            .arg(&missing)
            .output()
            .expect("runs animsmith");
        assert_eq!(output.status.code(), Some(2), "{name}: {}", stderr(&output));
        let error = stderr(&output);
        assert!(error.contains(expected), "{name}: {error}");
        assert!(!error.contains("failed to read"), "{name}: {error}");
    }
}

#[test]
fn engine_input_format_comes_from_the_loader_and_has_no_override() {
    let dir = unique_temp_dir("engine-profile-format");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let config = write_config(
        dir.path(),
        "unity.toml",
        r#"
[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
convert_units = true
bake_axis_conversion = true
root_motion_source = "Reference/Root"

[clips."*".engine_settings]
root_rotation = "extract"
root_position_y = "bake"
root_position_xz = "extract"
"#,
    );
    let output = animsmith()
        .args(["--config"])
        .arg(&config)
        .args(["measure"])
        .arg(&input)
        .output()
        .expect("runs animsmith");
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let error = stderr(&output);
    assert!(
        error.contains("input format Glb is not accepted"),
        "{error}"
    );
    assert!(!error.contains("source_unit"), "{error}");
}

#[test]
fn profiled_diff_validates_json_reports_without_consuming_prediction_meaning() {
    let dir = unique_temp_dir("engine-profile-report-format");
    let input = dir.path().join("sway.glb");
    let report = dir.path().join("sway.measure.json");
    write_clean_glb(&input);
    let measured = animsmith()
        .args(["measure"])
        .arg(&input)
        .args(["--format", "json"])
        .output()
        .expect("measures report input");
    assert_eq!(measured.status.code(), Some(0), "{}", stderr(&measured));
    std::fs::write(&report, measured.stdout).expect("writes measurement report");

    let config = write_config(
        dir.path(),
        "bevy.toml",
        r#"
[engine]
profile = "bevy"
profile_revision = 1
engine_version = "0.19.0"
importer = "gltf-asset-loader"
"#,
    );
    let baseline = animsmith()
        .args(["diff"])
        .arg(&report)
        .arg(&report)
        .args(["--format", "json"])
        .output()
        .expect("runs baseline report diff");
    assert_eq!(baseline.status.code(), Some(0), "{}", stderr(&baseline));
    let output = animsmith()
        .args(["--config"])
        .arg(&config)
        .args(["diff"])
        .arg(&report)
        .arg(&report)
        .args(["--format", "json"])
        .output()
        .expect("runs profiled report diff");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(output.stdout, baseline.stdout);
}

#[test]
fn diff_validates_full_prediction_provenance_before_measurements_and_rejects_unknown_fields() {
    let dir = unique_temp_dir("prediction-provenance-readback");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let config = write_config(
        dir.path(),
        "bevy.toml",
        r#"
[engine]
profile = "bevy"
profile_revision = 1
engine_version = "0.19.0"
importer = "gltf-asset-loader"
"#,
    );
    let lint = animsmith()
        .args(["--config"])
        .arg(&config)
        .args(["lint"])
        .arg(&input)
        .args(["--format", "json"])
        .output()
        .expect("runs profiled lint");
    assert_eq!(lint.status.code(), Some(0), "{}", stderr(&lint));
    let valid: Value = serde_json::from_slice(&lint.stdout).expect("valid lint JSON");

    let run_diff = |name: &str, report: &Value| {
        let path = dir.path().join(name);
        std::fs::write(
            &path,
            serde_json::to_vec(report).expect("serializes report"),
        )
        .expect("writes report");
        animsmith()
            .args(["diff"])
            .arg(&path)
            .arg(&path)
            .output()
            .expect("runs report diff")
    };

    let mut outer_precedence = valid.clone();
    outer_precedence["schema_version"] = json!(9);
    outer_precedence["files"][0]["prediction_provenance"]["unexpected"] = json!(true);
    let output = run_diff("invalid-outer-precedence.json", &outer_precedence);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let error = stderr(&output);
    assert!(error.contains("schema_version 9"), "{error}");
    assert!(!error.contains("prediction provenance"), "{error}");
    assert!(!error.contains("unexpected"), "{error}");

    let mut precedence = valid.clone();
    precedence["files"][0]["prediction_provenance"]["identity"]["sha256"] = json!("0".repeat(64));
    precedence["files"][0]["measurements"] = json!("not a measurement object");
    let output = run_diff("invalid-provenance-precedence.json", &precedence);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let error = stderr(&output);
    assert!(error.contains("invalid prediction provenance"), "{error}");
    assert!(!error.contains("provenance shape"), "{error}");
    assert!(
        error.contains("urn:animsmith:prediction-provenance:3"),
        "{error}"
    );
    assert!(error.contains("identity"), "{error}");
    assert!(!error.contains("invalid measurements shape"), "{error}");

    let mut malformed_measurements = valid.clone();
    malformed_measurements["files"][0]["measurements"] = json!("not a measurement object");
    let output = run_diff("invalid-measurements-shape.json", &malformed_measurements);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let error = stderr(&output);
    assert!(error.contains("invalid measurements shape"), "{error}");
    assert!(!error.contains("prediction provenance"), "{error}");

    let mut finding_parent = valid.clone();
    let parent_check_id = finding_parent["files"][0]["checks"][0]["check_id"]
        .as_str()
        .expect("serialized check id")
        .to_owned();
    finding_parent["files"][0]["checks"][0]["findings"] = json!([{
        "check_id": format!("{parent_check_id}-wrong"),
        "severity": "note",
        "message": "synthetic finding",
    }]);
    let output = run_diff("invalid-finding-parent.json", &finding_parent);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let error = stderr(&output);
    assert!(
        error.contains("finding check_id must match its parent check"),
        "{error}"
    );

    let mut missing_finding_parent = valid.clone();
    missing_finding_parent["files"][0]["checks"][0]["findings"] = json!([{
        "severity": "note",
        "message": "synthetic finding",
    }]);
    let output = run_diff("missing-finding-parent.json", &missing_finding_parent);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let error = stderr(&output);
    assert!(error.contains("invalid prediction shape"), "{error}");
    assert!(error.contains("missing field `check_id`"), "{error}");

    let mut unknown = valid;
    unknown["files"][0]["prediction_provenance"]["unexpected"] = json!(true);
    let output = run_diff("unknown-provenance-field.json", &unknown);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let error = stderr(&output);
    assert!(
        error.contains("invalid prediction provenance shape"),
        "{error}"
    );
    assert!(error.contains("unknown field `unexpected`"), "{error}");
}

#[test]
fn loop_continuity_clip_caps_layer_global_glob_and_exact_contracts() {
    let dir = unique_temp_dir("clip-loop-caps");
    let input = fixture("rig.gltf");
    for (name, text, expected_position, expected_rotation) in [
        (
            "global.toml",
            "[checks.loop-closure]\nmax_position_delta_m = 0.5\nmax_rotation_delta_deg = 80.0\n\n[clips.walk]\nloop = true\n",
            0.5,
            80.0,
        ),
        (
            "glob.toml",
            "[checks.loop-closure]\nmax_position_delta_m = 0.5\nmax_rotation_delta_deg = 80.0\n\n[clips.\"wa*\"]\nloop = true\nmax_loop_position_delta_m = 0.75\nmax_loop_rotation_delta_deg = 85.0\n",
            0.75,
            85.0,
        ),
        (
            "exact.toml",
            "[checks.loop-closure]\nmax_position_delta_m = 0.5\nmax_rotation_delta_deg = 80.0\n\n[clips.\"wa*\"]\nloop = true\nmax_loop_position_delta_m = 0.75\nmax_loop_rotation_delta_deg = 85.0\n\n[clips.walk]\nmax_loop_rotation_delta_deg = 87.0\n",
            0.75,
            87.0,
        ),
    ] {
        let config = write_config(dir.path(), name, text);
        let output = animsmith()
            .args(["--config"])
            .arg(&config)
            .args(["lint"])
            .arg(&input)
            .args(["--select", "loop-closure", "--format", "json"])
            .output()
            .expect("runs animsmith");
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        let closure = json["files"][0]["checks"]
            .as_array()
            .expect("checks array")
            .iter()
            .find(|check| check["check_id"] == "loop-closure")
            .expect("loop-closure record");
        let findings = closure["findings"].as_array().expect("findings array");
        assert_eq!(findings.len(), 2, "{json:#}");
        assert_eq!(findings[0]["expected"], expected_position, "{json:#}");
        assert_eq!(findings[1]["expected"], expected_rotation, "{json:#}");
    }
}

#[test]
fn invalid_clip_loop_caps_are_operator_errors() {
    let dir = unique_temp_dir("invalid-clip-loop-cap");
    let input = fixture("rig.gltf");
    for (name, key, value) in [
        ("negative", "max_loop_position_delta_m", "-0.01"),
        ("nan", "max_loop_rotation_delta_deg", "nan"),
        ("infinite", "max_loop_velocity_delta_mps", "inf"),
        (
            "angular-infinite",
            "max_loop_angular_velocity_delta_degps",
            "inf",
        ),
    ] {
        let config = write_config(
            dir.path(),
            &format!("{name}.toml"),
            &format!("[clips.walk]\nloop = true\n{key} = {value}\n"),
        );
        let output = animsmith()
            .args(["--config"])
            .arg(&config)
            .args(["lint"])
            .arg(&input)
            .output()
            .expect("runs animsmith");
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert!(
            stderr(&output).contains("must be a finite non-negative number"),
            "{name}: {}",
            stderr(&output)
        );
    }
}

/// The shipped example config must parse verbatim — otherwise it drifts
/// from the schema and fails users at runtime while CI stays green.
#[test]
fn example_config_parses_verbatim() {
    let config = example_config();
    assert!(config.exists(), "example config missing at {config:?}");
    let output = animsmith()
        .args([
            "--config",
            config.to_str().expect("utf-8 path"),
            "inspect",
            fixture("rig.gltf").to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "example config did not parse:\nstderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn inspect_reports_clip_and_profile() {
    let output = animsmith()
        .args(["inspect", fixture("rig.gltf").to_str().expect("utf-8 path")])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let out = stdout(&output);
    // Distinctive clip detail: the fixture's one clip, its duration and
    // track/key counts — pins that inspect actually read the file, not
    // just that it printed a static template.
    assert!(
        out.contains("walk: 1.000s, 2 tracks, 3 keys max"),
        "clip summary missing/changed:\n{out}"
    );
    assert!(out.contains("rig profile:"), "no profile line:\n{out}");
    assert!(
        out.contains("skeleton: 3 bones"),
        "no skeleton line:\n{out}"
    );
}

#[test]
fn inspect_reports_every_selectable_mesh_instance_and_material() {
    let dir = unique_temp_dir("inspect-mesh-instances");
    let input = dir.path().join("multi-mesh.glb");
    write_multi_mesh_glb(&input);

    let output = animsmith()
        .args(["inspect", input.to_str().expect("utf-8 path")])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let out = stdout(&output);
    let inventory = &out[out.find("materials:").expect("material inventory")..];
    assert_eq!(
        inventory,
        concat!(
            "materials: 5\n",
            "  #0 \"body-material\"\n",
            "  #1 \"prop-material\"\n",
            "  #2 \" duplicate-material \" [ambiguous: 3 materials share this name]\n",
            "  #3 \" duplicate-material \" [ambiguous: 3 materials share this name]\n",
            "  #4 \" duplicate-material \" [ambiguous: 3 materials share this name]\n",
            "mesh instances: 5\n",
            "  node \"body-node\"\n",
            "    source node: #0\n",
            "    mesh: #0 \"body-mesh\" (source mesh #0)\n",
            "    skin: unskinned\n",
            "    primitive #0: material #0 \"body-material\"\n",
            "  node \"prop-node\"\n",
            "    source node: #1\n",
            "    mesh: #1 \"prop-mesh\" (source mesh #1)\n",
            "    skin: unskinned\n",
            "    primitive #0: material #1 \"prop-material\"\n",
            "  node \" duplicate-node \" [ambiguous: 3 skeleton nodes share this name]\n",
            "    source node: #2\n",
            "    mesh: #0 \"body-mesh\" (source mesh #0)\n",
            "    skin: unskinned\n",
            "    primitive #0: material #0 \"body-material\"\n",
            "  node \" duplicate-node \" [ambiguous: 3 skeleton nodes share this name]\n",
            "    source node: #3\n",
            "    mesh: #0 \"body-mesh\" (source mesh #0)\n",
            "    skin: unskinned\n",
            "    primitive #0: material #0 \"body-material\"\n",
            "  node \" duplicate-node \" [ambiguous: 3 skeleton nodes share this name]\n",
            "    source node: #4\n",
            "    mesh: #0 \"body-mesh\" (source mesh #0)\n",
            "    skin: unskinned\n",
            "    primitive #0: material #0 \"body-material\"\n",
            "clips: 0\n",
        )
    );
}
