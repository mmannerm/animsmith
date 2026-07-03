//! Mesh/skin/material emission: a synthetic skinned triangle written
//! with `write_with_assets` must parse as valid glTF with every
//! attribute readable and weights normalized.

use animsmith_core::model::*;
use glam::{Mat4, Vec3};

fn skinned_triangle() -> (Document, SceneAssets) {
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![
                Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: Some(Mat4::IDENTITY),
                },
                Bone {
                    name: "tip".into(),
                    parent: Some(0),
                    rest: Transform {
                        translation: Vec3::new(0.0, 1.0, 0.0),
                        ..Transform::IDENTITY
                    },
                    inverse_bind: Some(Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0))),
                },
            ],
        },
        clips: vec![],
        source: SourceInfo::default(),
    };
    let assets = SceneAssets {
        meshes: vec![MeshAsset {
            name: "tri".into(),
            node: 0,
            primitives: vec![Primitive {
                material: Some(0),
                positions: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                ],
                normals: vec![Vec3::Z; 3],
                uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                joints: vec![[0, 1, 0, 0]; 3],
                weights: vec![[0.75, 0.25, 0.0, 0.0]; 3],
            }],
            skin_joints: vec![0, 1],
            skin_ibms: vec![],
        }],
        materials: vec![MaterialAsset {
            name: "mat".into(),
            base_color: [0.8, 0.2, 0.2, 1.0],
            metallic: 0.0,
            roughness: 0.9,
        }],
    };
    (doc, assets)
}

#[test]
fn skinned_mesh_round_trips_through_gltf_parser() {
    let dir = std::env::temp_dir().join("animsmith-assets-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tri.glb");
    let (doc, assets) = skinned_triangle();
    animsmith_gltf::write::write_with_assets(&doc, &assets, &path).expect("writes");

    let bytes = std::fs::read(&path).unwrap();
    let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");
    let blob = gltf.blob.clone().expect("BIN chunk");
    let get = |b: gltf::Buffer| -> Option<&[u8]> { Some(&blob[..b.length()]) };

    let mesh = gltf.meshes().next().expect("mesh present");
    let prim = mesh.primitives().next().expect("primitive present");
    assert_eq!(prim.material().index(), Some(0));
    let reader = prim.reader(get);
    let positions: Vec<[f32; 3]> = reader.read_positions().expect("POSITION").collect();
    assert_eq!(positions.len(), 3);
    assert_eq!(positions[1], [1.0, 0.0, 0.0]);
    assert_eq!(
        reader.read_normals().expect("NORMAL").count(),
        3,
        "normals present"
    );
    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .expect("TEXCOORD_0")
        .into_f32()
        .collect();
    assert_eq!(uvs[2], [0.0, 1.0]);
    let joints: Vec<[u16; 4]> = reader
        .read_joints(0)
        .expect("JOINTS_0")
        .into_u16()
        .collect();
    assert_eq!(joints[0], [0, 1, 0, 0]);
    let weights: Vec<[f32; 4]> = reader
        .read_weights(0)
        .expect("WEIGHTS_0")
        .into_f32()
        .collect();
    for w in &weights {
        let sum: f32 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "weights must normalize: {w:?}");
    }

    let skin = gltf.skins().next().expect("skin present");
    let joints: Vec<usize> = skin.joints().map(|j| j.index()).collect();
    assert_eq!(joints, vec![0, 1]);
    let ibms: Vec<[[f32; 4]; 4]> = skin
        .reader(get)
        .read_inverse_bind_matrices()
        .expect("IBMs")
        .collect();
    assert_eq!(ibms.len(), 2);
    assert_eq!(ibms[1][3][1], -1.0, "tip IBM carries the -1 y translation");

    // Skinned meshes hang off a dedicated identity node at scene root
    // (loader-compatibility rule), not the original bone node.
    let holder = gltf
        .nodes()
        .find(|n| n.mesh().is_some())
        .expect("mesh holder node");
    assert!(holder.skin().is_some());
    assert!(
        holder.transform().decomposed().0 == [0.0, 0.0, 0.0],
        "holder node must carry no transform"
    );
    let scene_roots: Vec<usize> = gltf
        .default_scene()
        .expect("scene")
        .nodes()
        .map(|n| n.index())
        .collect();
    assert!(
        scene_roots.contains(&holder.index()),
        "holder at scene root"
    );

    // POSITION accessor carries the spec-required min/max.
    let pos_accessor = prim.get(&gltf::Semantic::Positions).unwrap();
    assert!(pos_accessor.min().is_some() && pos_accessor.max().is_some());
}
