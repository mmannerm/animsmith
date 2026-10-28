use super::*;
use crate::{LoadedSource, load_source_bytes_with_budget, read_external_file};
use base64::Engine as _;
use glam::{Mat4, Quat, Vec3};
use serde_json::{Value, json};
use std::path::Path;

fn encoded(mut root: Value, bytes: &[u8]) -> Vec<u8> {
    root["asset"] = json!({"version":"2.0"});
    root["buffers"] = json!([{"byteLength": bytes.len(), "uri": format!(
        "data:application/octet-stream;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)
    )}]);
    serde_json::to_vec(&root).unwrap()
}

fn load(bytes: &[u8], storage: usize) -> Result<LoadedSource, LoadError> {
    load_source_bytes_with_budget(
        Path::new("budget.gltf"),
        bytes,
        None,
        read_external_file,
        &mut DecodedAccessorBudget::new(storage),
    )
}

fn refused(result: Result<LoadedSource, LoadError>, accessor: usize, slot: &str) {
    match result.unwrap_err() {
        LoadError::DecodedAccessorBudget {
            accessor: actual,
            location,
            ..
        } => {
            assert_eq!(actual, accessor);
            assert_eq!(location, slot);
        }
        other => panic!("expected located decoded-budget error, got {other}"),
    }
}

fn sparse_position(count: usize) -> Value {
    json!({"componentType":5126,"count":count,"type":"VEC3","min":[0,0,0],"max":[1,2,3],
        "sparse":{"count":1,"indices":{"bufferView":0,"componentType":5125},"values":{"bufferView":1}}})
}

#[test]
fn sparse_zeros_and_repeated_uninstanced_primitives_count_each_array() {
    let mut bytes = 1_u32.to_le_bytes().to_vec();
    for value in [1.0_f32, 2.0, 3.0] {
        bytes.extend(value.to_le_bytes());
    }
    let root = json!({
        "bufferViews":[{"buffer":0,"byteLength":4},{"buffer":0,"byteOffset":4,"byteLength":12}],
        "accessors":[sparse_position(3), sparse_position(32), sparse_position(64)],
        "meshes":[{"primitives":[
            {"attributes":{"POSITION":0}}, {"attributes":{"POSITION":0}}
        ]}, {"primitives":[{"mode":1,"attributes":{"POSITION":1}}]}]
    });
    let source = encoded(root, &bytes);
    refused(load(&source, 71), 0, "mesh 0 primitive 1 POSITION");
    let loaded = load(&source, 72).unwrap();
    assert!(loaded.document().assets.instances.is_empty());
    let meshes = &loaded.document().assets.meshes;
    assert_eq!(
        meshes.len(),
        1,
        "discarded lines do not spend decoded budget"
    );
    for primitive in &meshes[0].primitives {
        assert_eq!(
            primitive.positions,
            [Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0), Vec3::ZERO]
        );
    }
}

#[test]
fn decoded_storage_counts_widened_attributes_and_indices() {
    let mut bytes = vec![0_u8; 72]; // Three float positions and normals each.
    bytes.extend([0, 255, 255, 0, 0, 0, 0, 0]); // UVs plus alignment.
    bytes.extend([0, 1, 2, 3].repeat(3)); // u8 JOINTS_0.
    bytes.extend([255, 0, 0, 0].repeat(3)); // normalized u8 WEIGHTS_0.
    bytes.extend([0, 1, 2, 0]); // u8 indices plus alignment.
    let source = encoded(
        json!({
            "bufferViews":[
                {"buffer":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":36},
                {"buffer":0,"byteOffset":72,"byteLength":6},{"buffer":0,"byteOffset":80,"byteLength":12},
                {"buffer":0,"byteOffset":92,"byteLength":12},{"buffer":0,"byteOffset":104,"byteLength":3}
            ],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[0,0,0]},
                {"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"},
                {"bufferView":2,"componentType":5121,"count":3,"type":"VEC2","normalized":true},
                {"bufferView":3,"componentType":5121,"count":3,"type":"VEC4"},
                {"bufferView":4,"componentType":5121,"count":3,"type":"VEC4","normalized":true},
                {"bufferView":5,"componentType":5121,"count":3,"type":"SCALAR"}
            ],
            "meshes":[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2,"JOINTS_0":3,"WEIGHTS_0":4},"indices":5}]}]
        }),
        &bytes,
    );
    // 108 source bytes expand to 180 decoded bytes. Both failures are in
    // later arrays, so neither per-accessor nor source-byte accounting suffices.
    refused(load(&source, 167), 4, "mesh 0 primitive 0 WEIGHTS_0");
    refused(load(&source, 179), 5, "mesh 0 primitive 0 indices");
    let loaded = load(&source, 180).unwrap();
    let primitive = &loaded.document().assets.meshes[0].primitives[0];
    assert_eq!(primitive.uvs, [[0.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
    assert_eq!(primitive.joints, [[0, 1, 2, 3]; 3]);
    assert_eq!(primitive.weights, [[1.0, 0.0, 0.0, 0.0]; 3]);
    assert_eq!(primitive.indices, [0, 1, 2]);
}

pub(crate) fn repeated_animation(project_geometry: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in [0.0_f32, 1.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0] {
        bytes.extend(value.to_le_bytes());
    }
    let mut root = json!({
        "bufferViews":[{"buffer":0,"byteLength":8},{"buffer":0,"byteOffset":8,"byteLength":24}],
        "accessors":[
            {"bufferView":0,"componentType":5126,"count":2,"type":"SCALAR","min":[0],"max":[1]},
            {"bufferView":1,"componentType":5126,"count":2,"type":"VEC3"}
        ],
        "nodes":[{},{}],
        "animations":[{"samplers":[{"input":0,"output":1}],"channels":[
            {"sampler":0,"target":{"node":0,"path":"translation"}},
            {"sampler":0,"target":{"node":1,"path":"translation"}}
        ]}]
    });
    if project_geometry {
        // Sparse geometry is outside the strict scale capability, but a clip
        // projection discards it. Its decoded count exceeds our tiny budget.
        root["bufferViews"].as_array_mut().unwrap().extend([
            json!({"buffer":0,"byteOffset":32,"byteLength":4}),
            json!({"buffer":0,"byteOffset":36,"byteLength":12}),
        ]);
        let mut position = sparse_position(32);
        position["sparse"]["indices"]["bufferView"] = json!(2);
        position["sparse"]["values"]["bufferView"] = json!(3);
        root["accessors"].as_array_mut().unwrap().push(position);
        root["meshes"] = json!([{"primitives":[{"attributes":{"POSITION":2}}]}]);
        root["nodes"][0]["mesh"] = json!(0);
        bytes.extend([0_u8; 16]);
    }
    encoded(root, &bytes)
}

#[test]
fn channels_sharing_a_sampler_charge_every_timeline_and_output() {
    let source = repeated_animation(false);
    refused(load(&source, 63), 1, "animation 0 channel 1 node 1 output");
    let loaded = load(&source, 64).unwrap();
    let tracks = &loaded.document().clips[0].tracks;
    assert_eq!(tracks.len(), 2);
    for track in tracks {
        assert_eq!(track.times, [0.0, 1.0]);
        let animsmith_core::TrackValues::Vec3s(values) = &track.values else {
            panic!("translation must retain vector values");
        };
        assert_eq!(values, &[Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0)]);
    }
}

#[test]
fn source_skins_and_each_instance_copy_share_one_budget() {
    let mut bytes = Vec::new();
    for value in Mat4::IDENTITY
        .to_cols_array()
        .into_iter()
        .chain(Mat4::from_translation(Vec3::X).to_cols_array())
    {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend([0_u8; 36]);
    let source = encoded(
        json!({
            "bufferViews":[{"buffer":0,"byteLength":128},{"buffer":0,"byteOffset":128,"byteLength":36}],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":2,"type":"MAT4"},
                {"bufferView":1,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[0,0,0]}
            ],
            "skins":[{"joints":[0,1],"inverseBindMatrices":0},{"joints":[0,1],"inverseBindMatrices":0}],
            "nodes":[{},{},{"mesh":0,"skin":0},{"mesh":0,"skin":1}],
            "meshes":[{"primitives":[{"attributes":{"POSITION":1}}]}]
        }),
        &bytes,
    );
    refused(load(&source, 255), 0, "skin 1 inverseBindMatrices");
    refused(
        load(&source, 547),
        0,
        "node 3 skin 1 inverseBindMatrices copy",
    );
    let loaded = load(&source, 548).unwrap();
    let expected = [Mat4::IDENTITY, Mat4::from_translation(Vec3::X)];
    for skin in &loaded.document().assets.source_skeleton.skins {
        assert_eq!(skin.inverse_bind_accessor.matrices, expected);
    }
    for instance in &loaded.document().assets.instances {
        assert_eq!(instance.skin_ibms, expected);
    }
    assert_eq!(
        loaded.document().skeleton.bones[1].inverse_bind,
        Some(expected[1])
    );
}

#[test]
fn default_admits_exactly_256_mib_then_refuses_one_more_decoded_value() {
    // Admission only: no reader or allocation. Literal f32 counts pin the
    // documented 256 MiB policy independently of the implementation constant.
    let mut budget = DecodedAccessorBudget::default();
    budget.admit::<f32>(0, 67_108_864, "full").unwrap();
    assert!(budget.admit::<f32>(0, 1, "one more value").is_err());
}

#[test]
fn arithmetic_overflow_is_refused_without_allocating() {
    let mut budget = DecodedAccessorBudget::new(usize::MAX);
    assert!(
        budget
            .admit::<u32>(0, usize::MAX, "multiply overflow")
            .is_err()
    );
    let largest = isize::MAX as usize / 4;
    assert!(budget.admit::<u32>(0, largest + 1, "allocation").is_err());
    budget.admit::<u32>(0, largest, "first array").unwrap();
    budget.admit::<u32>(0, largest, "second array").unwrap();
    assert!(budget.admit::<u32>(0, 2, "aggregate overflow").is_err());
}

#[test]
fn quaternion_outputs_charge_four_components() {
    let mut bytes = Vec::new();
    for value in [0.0_f32, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0] {
        bytes.extend(value.to_le_bytes());
    }
    let source = encoded(
        json!({
            "bufferViews":[{"buffer":0,"byteLength":8},{"buffer":0,"byteOffset":8,"byteLength":32}],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":2,"type":"SCALAR","min":[0],"max":[1]},
                {"bufferView":1,"componentType":5126,"count":2,"type":"VEC4"}
            ],
            "nodes":[{}],
            "animations":[{"samplers":[{"input":0,"output":1}],"channels":[
                {"sampler":0,"target":{"node":0,"path":"rotation"}}
            ]}]
        }),
        &bytes,
    );
    // Eight timeline bytes plus two sixteen-byte quaternions.
    refused(load(&source, 39), 1, "animation 0 channel 0 node 0 output");
    let loaded = load(&source, 40).unwrap();
    let animsmith_core::TrackValues::Quats(values) = &loaded.document().clips[0].tracks[0].values
    else {
        panic!("rotation must retain quaternion values");
    };
    assert_eq!(
        values,
        &[Quat::IDENTITY, Quat::from_xyzw(0.0, 0.0, 1.0, 0.0)]
    );
}

#[test]
fn unavailable_inverse_binds_do_not_spend_budget() {
    let source = encoded(
        json!({
            "bufferViews":[{"buffer":0,"byteLength":4},{"buffer":0,"byteOffset":4,"byteLength":64}],
            "accessors":[{"componentType":5126,"count":8,"type":"MAT4",
                "sparse":{"count":1,"indices":{"bufferView":0,"componentType":5125},"values":{"bufferView":1}}}],
            "nodes":[{}], "skins":[{"joints":[0],"inverseBindMatrices":0}]
        }),
        &[0_u8; 68],
    );
    refused(load(&source, 64), 0, "skin 0 inverseBindMatrices");
    let mut truncated: Value = serde_json::from_slice(&source).unwrap();
    // Keep the declared lengths, but make the actual sparse value unavailable.
    truncated["buffers"][0]["uri"] = json!("data:application/octet-stream;base64,AAAAAA==");
    let loaded = load(&serde_json::to_vec(&truncated).unwrap(), 0).unwrap();
    let bind = &loaded.document().assets.source_skeleton.skins[0].inverse_bind_accessor;
    assert_eq!(
        bind.status,
        animsmith_core::SourceInverseBindAccessorStatus::Unreadable
    );
    assert_eq!(bind.declared_count, Some(8));
    assert!(bind.matrices.is_empty());
}
