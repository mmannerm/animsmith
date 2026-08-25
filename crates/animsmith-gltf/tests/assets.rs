//! Mesh/skin/material emission: a synthetic skinned triangle carried in
//! `Document::assets` must parse as valid glTF with every attribute
//! readable and weights normalized.

use animsmith_core::model::*;
use base64::Engine as _;
use glam::{Mat4, Quat, Vec3};

/// A valid 1×1 white PNG.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xA7, 0x35, 0x81, 0x84, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// A second valid 1x1 white PNG with distinct encoded bytes.
const TINY_NORMAL_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0xF8, 0xFF, 0xFF, 0xFF,
    0x7F, 0x00, 0x09, 0xFB, 0x03, 0xFD, 0xF5, 0xD8, 0xF1, 0x9A, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// Produces a valid PNG with a slot-specific ancillary text chunk.  Keeping
/// the image content tiny makes the material round-trip test focused, while
/// the encoded payload still distinguishes every PBR texture slot.
fn png_with_slot_label(label_chunk: &[u8]) -> Vec<u8> {
    let mut bytes = TINY_PNG[..TINY_PNG.len() - 12].to_vec();
    bytes.extend_from_slice(label_chunk);
    bytes.extend_from_slice(&TINY_PNG[TINY_PNG.len() - 12..]);
    bytes
}

fn skinned_triangle() -> Document {
    let skeleton = Skeleton {
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
    };
    let assets = SceneAssets {
        meshes: vec![MeshAsset {
            name: "tri".into(),
            source_mesh_index: 0,
            primitives: vec![{
                // Two triangles sharing an edge: 6 corners, 4 unique.
                let mut prim = Primitive {
                    material: Some(0),
                    indices: vec![],
                    positions: vec![
                        Vec3::new(0.0, 0.0, 0.0),
                        Vec3::new(1.0, 0.0, 0.0),
                        Vec3::new(0.0, 1.0, 0.0),
                        Vec3::new(1.0, 0.0, 0.0),
                        Vec3::new(1.0, 1.0, 0.0),
                        Vec3::new(0.0, 1.0, 0.0),
                    ],
                    normals: vec![Vec3::Z; 6],
                    uvs: vec![
                        [0.0, 0.0],
                        [1.0, 0.0],
                        [0.0, 1.0],
                        [1.0, 0.0],
                        [1.0, 1.0],
                        [0.0, 1.0],
                    ],
                    joints: vec![[0, 1, 0, 0]; 6],
                    weights: vec![[0.75, 0.25, 0.0, 0.0]; 6],
                    ..Primitive::default()
                };
                prim.weld();
                prim
            }],
        }],
        instances: vec![MeshInstance {
            source_node_index: 0,
            node: 0,
            mesh: 0,
            skin_joints: vec![0, 1],
            skin_ibms: vec![],
        }],
        materials: vec![MaterialAsset {
            name: "mat".into(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.9,
            base_color_texture: Some(TextureAsset {
                bytes: TINY_PNG.to_vec(),
                mime: "image/png".into(),
            }),
            normal_texture: None,
            metallic_roughness_texture: None,
            occlusion_texture: None,
        }],
        ..SceneAssets::default()
    };
    Document {
        skeleton,
        clips: vec![],
        assets,
        source: SourceInfo::default(),
    }
}

#[test]
fn skinned_mesh_round_trips_through_gltf_parser() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tri.glb");
    let doc = skinned_triangle();
    let summary = animsmith_gltf::write::write(&doc, &path).expect("writes");

    let bytes = std::fs::read(&path).unwrap();
    let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");
    assert_eq!(
        (summary.nodes, gltf.nodes().count()),
        (doc.skeleton.bones.len() + 1, doc.skeleton.bones.len() + 1),
        "summary and raw glTF include the skinned-mesh holder node"
    );
    let blob = gltf.blob.clone().expect("BIN chunk");
    let get = |b: gltf::Buffer| -> Option<&[u8]> { Some(&blob[..b.length()]) };

    let mesh = gltf.meshes().next().expect("mesh present");
    let prim = mesh.primitives().next().expect("primitive present");
    assert_eq!(prim.material().index(), Some(0));
    let reader = prim.reader(get);
    let positions: Vec<[f32; 3]> = reader.read_positions().expect("POSITION").collect();
    assert_eq!(positions.len(), 4, "welded to unique corners");
    assert_eq!(positions[1], [1.0, 0.0, 0.0]);
    let indices: Vec<u32> = reader.read_indices().expect("indices").into_u32().collect();
    assert_eq!(indices.len(), 6, "two triangles");
    assert_eq!(&indices[..3], &[0, 1, 2]);
    assert_eq!(
        reader.read_normals().expect("NORMAL").count(),
        4,
        "normals present"
    );
    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .expect("TEXCOORD_0")
        .into_f32()
        .collect();
    assert_eq!(uvs[2], [0.0, 1.0]);
    // The embedded texture round-trips byte-for-byte.
    let material = prim.material();
    let tex = material
        .pbr_metallic_roughness()
        .base_color_texture()
        .expect("baseColorTexture")
        .texture();
    let image = tex.source().source();
    let gltf::image::Source::View { view, mime_type } = image else {
        panic!("image must be buffer-backed");
    };
    assert_eq!(mime_type, "image/png");
    let image_bytes = &blob[view.offset()..view.offset() + view.length()];
    assert_eq!(image_bytes, TINY_PNG);
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

#[test]
fn material_texture_slots_round_trip_independently() {
    for (label, has_base_color, has_normal, has_metallic_roughness, has_occlusion) in [
        ("no-textures", false, false, false, false),
        ("base-color-only", true, false, false, false),
        ("normal-only", false, true, false, false),
        ("metallic-roughness-only", false, false, true, false),
        ("occlusion-only", false, false, false, true),
        ("all-textures", true, true, true, true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("{label}.glb"));
        let metallic_roughness_png = png_with_slot_label(&[
            0x00, 0x00, 0x00, 0x17, 0x74, 0x45, 0x58, 0x74, 0x73, 0x6C, 0x6F, 0x74, 0x3D, 0x6D,
            0x65, 0x74, 0x61, 0x6C, 0x6C, 0x69, 0x63, 0x2D, 0x72, 0x6F, 0x75, 0x67, 0x68, 0x6E,
            0x65, 0x73, 0x73, 0x52, 0x45, 0x7E, 0x42,
        ]);
        let occlusion_png = png_with_slot_label(&[
            0x00, 0x00, 0x00, 0x0E, 0x74, 0x45, 0x58, 0x74, 0x73, 0x6C, 0x6F, 0x74, 0x3D, 0x6F,
            0x63, 0x63, 0x6C, 0x75, 0x73, 0x69, 0x6F, 0x6E, 0xEF, 0x29, 0x18, 0x05,
        ]);
        let mut doc = skinned_triangle();
        let material = &mut doc.assets.materials[0];
        material.base_color_texture = has_base_color.then(|| TextureAsset {
            bytes: TINY_PNG.to_vec(),
            mime: "image/png".into(),
        });
        material.normal_texture = has_normal.then(|| NormalTextureAsset {
            texture: TextureAsset {
                bytes: TINY_NORMAL_PNG.to_vec(),
                mime: "image/png".into(),
            },
            scale: 0.375,
        });
        material.metallic_roughness_texture = has_metallic_roughness.then(|| TextureAsset {
            bytes: metallic_roughness_png.clone(),
            mime: "image/png".into(),
        });
        material.occlusion_texture = has_occlusion.then(|| OcclusionTextureAsset {
            texture: TextureAsset {
                bytes: occlusion_png.clone(),
                mime: "image/png".into(),
            },
            strength: 0.625,
        });

        animsmith_gltf::write::write(&doc, &path).expect("writes");

        let bytes = std::fs::read(&path).unwrap();
        let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");
        let raw = gltf.materials().next().expect("material");
        assert_eq!(
            raw.pbr_metallic_roughness().base_color_texture().is_some(),
            has_base_color,
            "{label}: raw base-color slot"
        );
        assert_eq!(
            raw.normal_texture().map(|normal| normal.scale()),
            has_normal.then_some(0.375),
            "{label}: raw normal slot and scale"
        );
        assert_eq!(
            raw.pbr_metallic_roughness()
                .metallic_roughness_texture()
                .is_some(),
            has_metallic_roughness,
            "{label}: raw metallic-roughness slot"
        );
        assert_eq!(
            raw.occlusion_texture()
                .map(|occlusion| occlusion.strength()),
            has_occlusion.then_some(0.625),
            "{label}: raw occlusion slot and strength"
        );
        assert_eq!(
            gltf.images().count(),
            usize::from(has_base_color)
                + usize::from(has_normal)
                + usize::from(has_metallic_roughness)
                + usize::from(has_occlusion),
            "{label}: independent images"
        );

        let loaded = animsmith_gltf::load(&path).expect("reloads");
        let material = &loaded.assets.materials[0];
        assert_eq!(
            material
                .base_color_texture
                .as_ref()
                .map(|texture| texture.bytes.as_slice()),
            has_base_color.then_some(TINY_PNG),
            "{label}: base-color bytes"
        );
        assert_eq!(
            material
                .normal_texture
                .as_ref()
                .map(|normal| (normal.texture.bytes.as_slice(), normal.scale)),
            has_normal.then_some((TINY_NORMAL_PNG, 0.375)),
            "{label}: normal bytes and scale"
        );
        assert_eq!(
            material
                .metallic_roughness_texture
                .as_ref()
                .map(|texture| texture.bytes.as_slice()),
            has_metallic_roughness.then_some(metallic_roughness_png.as_slice()),
            "{label}: metallic-roughness bytes"
        );
        assert_eq!(
            material
                .occlusion_texture
                .as_ref()
                .map(|occlusion| (occlusion.texture.bytes.as_slice(), occlusion.strength)),
            has_occlusion.then_some((occlusion_png.as_slice(), 0.625)),
            "{label}: occlusion bytes and strength"
        );
    }
}

/// `write` is driven entirely by `Document::assets`: clearing them (what
/// `convert --animation-only` does, uniformly across formats) yields a
/// mesh/skin/material-free file — while the same document's skeleton and
/// clips still write. This is the unification's data-loss contract: only
/// clearing assets drops geometry, never the write path itself.
#[test]
fn clearing_assets_writes_no_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stripped.glb");

    let mut doc = skinned_triangle();
    assert!(!doc.assets.meshes.is_empty(), "fixture carries a mesh");
    doc.assets = SceneAssets::default();
    animsmith_gltf::write::write(&doc, &path).expect("writes");

    let bytes = std::fs::read(&path).unwrap();
    let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");
    assert_eq!(gltf.meshes().count(), 0, "no meshes without assets");
    assert_eq!(gltf.skins().count(), 0, "no skins without assets");
    assert_eq!(gltf.materials().count(), 0, "no materials without assets");
    // The skeleton (both bones) still writes — geometry is the only loss.
    assert_eq!(gltf.nodes().count(), 2, "skeleton nodes survive");
}

/// A transform pass over a geometry-carrying `Document` must preserve
/// that geometry — this is the mechanism the `transform` CLI uses
/// (load → mutate clips → `write`), minus the file load. It pins the
/// data-loss regression #33 fixes: a transform that cleared or bypassed
/// `Document::assets` before writing would drop the mesh here, while a
/// correct one emits the (mutated) animation *and* the mesh together.
#[test]
fn transform_pass_preserves_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("transformed.glb");

    // A two-key rotation clip on the root bone, carried alongside the
    // skinned mesh.
    let mut doc = skinned_triangle();
    doc.clips = vec![Clip {
        name: "spin".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![Quat::IDENTITY, Quat::from_rotation_y(1.0)]),
        }],
    }];

    // Mutate the clip the way `transform` does; hold-extend appends one
    // key (2 → 3), so the emitted sampler input proves the pass ran.
    animsmith_core::transform::hold_extend(&mut doc.clips[0], 0.5);
    animsmith_gltf::write::write(&doc, &path).expect("writes");

    let bytes = std::fs::read(&path).unwrap();
    let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");

    // Geometry survived the transform pass.
    assert_eq!(gltf.meshes().count(), 1, "mesh survives a transform pass");
    assert_eq!(gltf.skins().count(), 1, "skin survives a transform pass");
    // The transform actually ran: the animation is present with the
    // hold-extended keyframe count.
    let anim = gltf.animations().next().expect("animation present");
    let input_keys = anim
        .samplers()
        .next()
        .expect("sampler present")
        .input()
        .count();
    assert_eq!(input_keys, 3, "hold-extend appended a keyframe");
}

/// The read side of the writer: `write` → `load` recovers meshes,
/// skins, and materials in the document's `assets`. This is the #16
/// round-trip contract — a GLB-based measure wrapper can now reach
/// geometry through animsmith. Every field is asserted whole (not a
/// sampled subset), so a loader that corrupted any unchecked position,
/// index, normal, UV, joint, weight, or IBM entry would fail here.
#[test]
fn load_round_trips_meshes_skins_materials() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("roundtrip.glb");
    animsmith_gltf::write::write(&skinned_triangle(), &path).expect("writes");

    let doc = animsmith_gltf::load(&path).expect("loads");
    let assets = &doc.assets;

    assert_eq!(assets.meshes.len(), 1, "one mesh");
    let mesh = &assets.meshes[0];
    let instance = &assets.instances[0];
    assert_eq!(mesh.primitives.len(), 1, "one primitive");
    let prim = &mesh.primitives[0];

    // Geometry recovers exactly as written — the fixture is welded to 4
    // unique corners / 6 indices before writing, and every entry is
    // pinned (glam/array types are `PartialEq`, so these compare whole).
    assert_eq!(
        prim.positions,
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
        ],
    );
    assert_eq!(prim.indices, vec![0, 1, 2, 1, 3, 2]);
    assert_eq!(prim.normals, vec![Vec3::Z; 4]);
    assert_eq!(
        prim.uvs,
        vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]
    );
    assert_eq!(prim.joints, vec![[0, 1, 0, 0]; 4]);
    assert_eq!(prim.weights, vec![[0.75, 0.25, 0.0, 0.0]; 4]);
    assert_eq!(prim.material, Some(0));

    // Skin: joints in cluster order, and both inverse bind matrices whole
    // (the writer falls back to the bones' `inverse_bind` when the mesh
    // carries none, so `tip` keeps its −1 y translation).
    assert_eq!(instance.skin_joints, vec![0, 1]);
    assert_eq!(
        instance.skin_ibms,
        vec![
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)),
        ],
    );

    // The skinned mesh hangs off the writer's dedicated holder node —
    // a synthesized 3rd bone — not the original bone-0 node it was
    // authored on. This pins the `MeshInstance::node` round-trip claim.
    assert_eq!(doc.skeleton.bones.len(), 3, "2 skeleton bones + holder");
    assert_eq!(instance.node, 2, "mesh maps to the holder bone, not bone 0");

    // Material: PBR factors and the embedded base-color texture, whole.
    assert_eq!(assets.materials.len(), 1);
    let material = &assets.materials[0];
    assert_eq!(material.base_color, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(material.metallic, 0.0);
    assert_eq!(material.roughness, 0.9);
    let texture = material
        .base_color_texture
        .as_ref()
        .expect("base-color texture present");
    assert_eq!(texture.mime, "image/png");
    assert_eq!(texture.bytes, TINY_PNG, "texture bytes round-trip");
}

/// The `.gltf` (text) path resolves a data-URI buffer and a
/// bufferView-backed image just like the binary `.glb` path.
#[test]
fn load_reads_gltf_text_format() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("roundtrip.gltf");
    animsmith_gltf::write::write(&skinned_triangle(), &path).expect("writes");

    let doc = animsmith_gltf::load(&path).expect("loads");
    let assets = &doc.assets;
    assert_eq!(assets.meshes.len(), 1);
    assert_eq!(assets.meshes[0].primitives[0].positions.len(), 4);
    assert_eq!(
        assets.materials[0]
            .base_color_texture
            .as_ref()
            .expect("texture")
            .bytes,
        TINY_PNG,
    );
}

/// An unindexed primitive stays unindexed: the writer emits no index
/// accessor, and `load` recovers the raw corner stream. This is the
/// "meshes (indexed or not)" half of #16 — every attribute (positions,
/// normals, UVs, joints, weights) is pinned per corner with distinct
/// values, so an unindexed loader that dropped or scrambled any of them
/// would fail here.
#[test]
fn load_preserves_unindexed_primitives() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unindexed.glb");

    // A single unwelded triangle: 3 distinct corners, no indices, every
    // optional attribute present with per-corner-distinct values.
    let positions = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let normals = vec![
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let uvs = vec![[0.0, 0.0], [1.0, 0.0], [0.25, 0.75]];
    let joints = vec![[0, 0, 0, 0], [1, 2, 0, 0], [3, 0, 0, 0]];
    let weights = vec![
        [1.0, 0.0, 0.0, 0.0],
        [0.5, 0.5, 0.0, 0.0],
        [0.25, 0.25, 0.5, 0.0],
    ];
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![],
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "tri".into(),
                source_mesh_index: 0,
                primitives: vec![Primitive {
                    material: None,
                    indices: vec![],
                    positions: positions.clone(),
                    normals: normals.clone(),
                    uvs: uvs.clone(),
                    joints: joints.clone(),
                    weights: weights.clone(),
                    ..Primitive::default()
                }],
            }],
            instances: vec![MeshInstance {
                source_node_index: 0,
                node: 0,
                mesh: 0,
                ..MeshInstance::default()
            }],
            materials: vec![],
            ..SceneAssets::default()
        },
        source: SourceInfo::default(),
    };
    animsmith_gltf::write::write(&doc, &path).expect("writes");

    let doc = animsmith_gltf::load(&path).expect("loads");
    let prim = &doc.assets.meshes[0].primitives[0];
    assert!(prim.indices.is_empty(), "no index accessor was emitted");
    // Every attribute round-trips exactly, per corner.
    assert_eq!(prim.positions, positions);
    assert_eq!(prim.normals, normals);
    assert_eq!(prim.uvs, uvs);
    assert_eq!(prim.joints, joints);
    assert_eq!(prim.weights, weights);
    assert_eq!(prim.material, None, "no material referenced");
}

/// Skin joints are node indices in the file; `load` must remap them to
/// bone ids through the *same* topological order the skeleton is built
/// in. This fixture orders the child node *before* its parent in the
/// glTF `nodes` array, so bone id ≠ node index (DFS-from-root makes the
/// root bone 0, the child bone 1). A loader that returned raw joint node
/// indices — or that let `extract_assets` diverge from `build_document`
/// — would produce `[0, 1]` instead of the correct `[1, 0]`.
#[test]
fn load_remaps_skin_joints_through_topological_bone_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reordered-skin.gltf");
    // buffer = 3 positions (VEC3 f32): (0,0,0),(1,0,0),(0,1,0).
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 36, "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }],
            "accessors": [{
                "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0]
            }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
            "nodes": [
                { "name": "child" },
                { "name": "root", "children": [0], "mesh": 0, "skin": 0 }
            ],
            "skins": [{ "joints": [0, 1] }],
            "scenes": [{ "nodes": [1] }],
            "scene": 0
        }"#,
    )
    .unwrap();

    let doc = animsmith_gltf::load(&path).expect("loads");
    // Topological bone order: root first (bone 0), child second (bone 1).
    assert_eq!(doc.skeleton.bones[0].name, "root");
    assert_eq!(doc.skeleton.bones[1].name, "child");
    assert_eq!(
        doc.skeleton.bones[1].parent,
        Some(0),
        "child hangs off root"
    );
    // Skin joints [node0=child, node1=root] remap to bone ids [1, 0] —
    // and match the skeleton the other code path built.
    assert_eq!(
        doc.assets.instances[0].skin_joints,
        vec![1, 0],
        "joints remapped to bone ids, both loader paths agreeing"
    );
    // The mesh hangs off node 1 (root), which is bone 0 — `MeshInstance::node`
    // is likewise a bone id, not the raw node index.
    assert_eq!(
        doc.assets.instances[0].node, 0,
        "MeshInstance::node is the remapped bone id"
    );
}

/// Mesh definitions remain distinct from node instances: two nodes can
/// reference one definition, while a different valid definition can have no
/// node reference at all. Declared scene roots are preserved independently of
/// the all-node skeleton topology.
#[test]
fn load_preserves_mesh_definition_instance_and_scene_identity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("instanced-scenes.gltf");
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 36, "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }],
            "accessors": [{
                "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0]
            }],
            "meshes": [
                { "name": "shared", "primitives": [{ "attributes": { "POSITION": 0 } }] },
                { "name": "uninstanced", "primitives": [{ "attributes": { "POSITION": 0 } }] }
            ],
            "nodes": [
                { "name": "left", "mesh": 0 },
                { "name": "right", "mesh": 0 }
            ],
            "scenes": [
                { "name": "left-scene", "nodes": [0] },
                { "name": "right-scene", "nodes": [1] }
            ],
            "scene": 1
        }"#,
    )
    .unwrap();

    let doc = animsmith_gltf::load(&path).expect("loads");
    assert_eq!(
        doc.assets.meshes.len(),
        2,
        "uninstanced definition retained"
    );
    assert_eq!(
        doc.assets
            .meshes
            .iter()
            .map(|mesh| (mesh.name.as_str(), mesh.source_mesh_index))
            .collect::<Vec<_>>(),
        vec![("shared", 0), ("uninstanced", 1)]
    );
    assert_eq!(doc.assets.instances.len(), 2);
    assert_eq!(
        doc.assets
            .instances
            .iter()
            .map(|instance| (instance.source_node_index, instance.node, instance.mesh))
            .collect::<Vec<_>>(),
        vec![(0, 0, 0), (1, 1, 0)]
    );
    assert_eq!(doc.assets.default_scene, Some(1));
    assert_eq!(doc.assets.scenes.len(), 2);
    assert_eq!(doc.assets.scenes[0].source_scene_index, 0);
    assert_eq!(doc.assets.scenes[0].name.as_deref(), Some("left-scene"));
    assert_eq!(doc.assets.scenes[0].roots, vec![0]);
    assert_eq!(doc.assets.scenes[1].source_scene_index, 1);
    assert_eq!(doc.assets.scenes[1].name.as_deref(), Some("right-scene"));
    assert_eq!(doc.assets.scenes[1].roots, vec![1]);
}

/// Absence of glTF's optional top-level `scene` property is distinct from
/// explicitly selecting scene zero.
#[test]
fn load_preserves_absent_default_scene() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("no-default-scene.gltf");
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "nodes": [{ "name": "root" }],
            "scenes": [{ "name": "declared-not-default", "nodes": [0] }]
        }"#,
    )
    .unwrap();

    let doc = animsmith_gltf::load(&path).expect("loads");
    assert_eq!(doc.assets.scenes.len(), 1, "declared scene is retained");
    assert_eq!(
        doc.assets.default_scene, None,
        "missing `scene` must not imply scene zero"
    );
}

/// Unskinned mesh definitions can be attached to more than one node. Writing
/// must emit one definition with both node attachments, and loading must keep
/// each instance's skeleton node (and therefore its authored transform).
#[test]
fn write_load_preserves_unskinned_mesh_instances_and_node_transforms() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("instanced-unskinned.glb");
    let left_rest = Transform {
        translation: Vec3::new(-2.0, 1.0, 0.5),
        rotation: Quat::from_rotation_z(0.25),
        scale: Vec3::new(1.0, 2.0, 1.0),
    };
    let right_rest = Transform {
        translation: Vec3::new(3.0, -1.0, 2.0),
        rotation: Quat::from_rotation_y(-0.5),
        scale: Vec3::new(0.5, 1.0, 1.5),
    };
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![
                Bone {
                    name: "left".into(),
                    parent: None,
                    rest: left_rest,
                    inverse_bind: None,
                },
                Bone {
                    name: "right".into(),
                    parent: None,
                    rest: right_rest,
                    inverse_bind: None,
                },
            ],
        },
        clips: vec![],
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "shared-triangle".into(),
                source_mesh_index: 0,
                primitives: vec![Primitive {
                    positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
                    ..Primitive::default()
                }],
            }],
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
                    mesh: 0,
                    ..MeshInstance::default()
                },
            ],
            ..SceneAssets::default()
        },
        source: SourceInfo::default(),
    };

    animsmith_gltf::write::write(&doc, &path).expect("writes");
    let loaded = animsmith_gltf::load(&path).expect("loads");

    assert_eq!(loaded.assets.meshes.len(), 1, "one shared definition");
    assert_eq!(loaded.assets.instances.len(), 2, "both attachments survive");
    assert_eq!(
        loaded
            .assets
            .instances
            .iter()
            .map(|instance| (instance.source_node_index, instance.node, instance.mesh))
            .collect::<Vec<_>>(),
        vec![(0, 0, 0), (1, 1, 0)],
    );
    assert_eq!(loaded.skeleton.bones.len(), 2);
    assert_eq!(loaded.skeleton.bones[0].name, "left");
    assert_eq!(loaded.skeleton.bones[0].rest, left_rest);
    assert_eq!(loaded.skeleton.bones[1].name, "right");
    assert_eq!(loaded.skeleton.bones[1].rest, right_rest);
}

/// `load` carries scene geometry in `Document::assets` — the same
/// one-call contract as `animsmith_fbx::load`, so consumers reach
/// meshes/materials without a second entry point.
#[test]
fn load_carries_assets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("has-geometry.glb");
    animsmith_gltf::write::write(&skinned_triangle(), &path).expect("writes");

    let doc = animsmith_gltf::load(&path).expect("loads");
    assert_eq!(doc.assets.meshes.len(), 1, "load() carries the mesh");
    assert_eq!(doc.assets.materials.len(), 1, "load() carries the material");
    assert!(!doc.skeleton.bones.is_empty(), "skeleton still loads");
}

/// `TINY_PNG` as a standard-base64 payload, for hand-authored `data:`
/// URI image sources (the writer only ever emits bufferView images, so
/// this branch needs a fixture that doesn't come from `write`).
const TINY_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4//8/AAX+Av6nNYGEAAAAAElFTkSuQmCC";

/// A base-color texture whose image is an embedded `data:` URI — the
/// `Source::Uri` data-URI branch of `read_image`, which no writer-driven
/// round-trip exercises (the writer always emits bufferView images). The
/// MIME is recovered from the URI's media-type prefix (no `mimeType`).
#[test]
fn load_reads_data_uri_texture() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data-uri-image.gltf");
    let json = format!(
        r#"{{
            "asset": {{ "version": "2.0" }},
            "images": [{{ "uri": "data:image/png;base64,{TINY_PNG_B64}" }}],
            "textures": [{{ "source": 0 }}],
            "materials": [{{
                "name": "mat",
                "pbrMetallicRoughness": {{ "baseColorTexture": {{ "index": 0 }} }}
            }}]
        }}"#
    );
    std::fs::write(&path, json).unwrap();

    let doc = animsmith_gltf::load(&path).expect("loads");
    let assets = &doc.assets;
    let texture = assets.materials[0]
        .base_color_texture
        .as_ref()
        .expect("data-URI texture recovered");
    assert_eq!(texture.mime, "image/png", "MIME parsed from the data URI");
    assert_eq!(texture.bytes, TINY_PNG, "decoded bytes match the source");
}

#[test]
fn load_reads_normal_texture_and_scale() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("normal-data-uri.gltf");
    let json = format!(
        r#"{{
            "asset": {{ "version": "2.0" }},
            "images": [{{ "uri": "data:image/png;base64,{TINY_PNG_B64}" }}],
            "textures": [{{ "source": 0 }}],
            "materials": [{{
                "name": "normal-mat",
                "normalTexture": {{ "index": 0, "scale": 0.625 }}
            }}]
        }}"#
    );
    std::fs::write(&path, json).unwrap();

    let doc = animsmith_gltf::load(&path).expect("loads");
    let normal = doc.assets.materials[0]
        .normal_texture
        .as_ref()
        .expect("normal texture recovered");
    assert_eq!(normal.texture.mime, "image/png");
    assert_eq!(normal.texture.bytes, TINY_PNG);
    assert_eq!(normal.scale, 0.625);
    assert!(doc.assets.materials[0].base_color_texture.is_none());
}

/// A base-color texture whose image is an external sibling file — the
/// `Source::Uri` external-file branch of `read_image`, read relative to
/// the glTF via the same containment rule as external buffers.
#[test]
fn load_reads_external_file_texture() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("tex map.png"), TINY_PNG).unwrap();
    let path = dir.path().join("external-image.gltf");
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "images": [{ "uri": "tex%20map.png", "mimeType": "image/png" }],
            "textures": [{ "source": 0 }],
            "materials": [{
                "name": "mat",
                "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } }
            }]
        }"#,
    )
    .unwrap();

    let doc = animsmith_gltf::load(&path).expect("loads");
    let assets = &doc.assets;
    let texture = assets.materials[0]
        .base_color_texture
        .as_ref()
        .expect("external-file texture recovered");
    assert_eq!(texture.mime, "image/png");
    assert_eq!(texture.bytes, TINY_PNG, "sibling PNG read from disk");
}

/// A bufferView-backed image whose `byteOffset + byteLength` overflows
/// `usize` must not crash the loader — hostile input yields an absent
/// texture, never a panic (invariant: loaders never panic on file data).
/// Without the checked add this panics in debug builds.
#[test]
fn load_survives_overflowing_image_view() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("overflow-view.gltf");
    // byteOffset = u64::MAX, byteLength = 1 → the sum overflows usize.
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 4, "uri": "data:application/octet-stream;base64,AAAAAA==" }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 18446744073709551615, "byteLength": 1 }],
            "images": [{ "bufferView": 0, "mimeType": "image/png" }],
            "textures": [{ "source": 0 }],
            "materials": [{
                "name": "m",
                "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } }
            }]
        }"#,
    )
    .unwrap();

    // The file must parse (so the overflowing view actually reaches
    // `read_image`); the unresolvable texture is simply dropped.
    let doc = animsmith_gltf::load(&path).expect("hostile view parses without panic");
    let assets = &doc.assets;
    assert!(
        assets.materials[0].base_color_texture.is_none(),
        "overflowing bufferView yields no texture, not a panic"
    );
}

/// A count-0 `POSITION` accessor must not crash the loader: gltf 1.4's
/// accessor iterator underflows (panics) on a zero-count accessor — the
/// same class of bug the animation path guards before reading. `load`
/// skips the empty primitive and returns a mesh-less document, never a
/// panic (invariant: hostile input never crashes the loader).
#[test]
fn load_survives_zero_count_position_accessor() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zero-count-position.gltf");
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 12, "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAA" }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 12 }],
            "accessors": [{
                "bufferView": 0, "componentType": 5126, "count": 0, "type": "VEC3",
                "min": [0.0, 0.0, 0.0], "max": [0.0, 0.0, 0.0]
            }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
            "nodes": [{ "mesh": 0 }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }"#,
    )
    .unwrap();

    let doc = animsmith_gltf::load(&path).expect("zero-count POSITION parses without panic");
    assert!(
        doc.assets.meshes.is_empty(),
        "the empty primitive is skipped, not crashed into"
    );
}

/// The zero-count guard covers *optional* accessors too: a valid
/// POSITION beside a count-0 NORMAL must load the geometry and drop the
/// empty attribute, never panic in the NORMAL reader.
#[test]
fn load_survives_zero_count_optional_accessor() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zero-count-normal.gltf");
    // buffer = 3 positions (0,0,0),(1,0,0),(0,1,0); NORMAL accessor is
    // count 0 over the same view.
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 36, "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0,0,0], "max": [1,1,0] },
                { "bufferView": 0, "componentType": 5126, "count": 0, "type": "VEC3" }
            ],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0, "NORMAL": 1 } }] }],
            "nodes": [{ "mesh": 0 }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }"#,
    )
    .unwrap();

    let doc = animsmith_gltf::load(&path).expect("zero-count NORMAL parses without panic");
    let prim = &doc.assets.meshes[0].primitives[0];
    assert_eq!(prim.positions.len(), 3, "POSITION still loaded");
    assert!(
        prim.normals.is_empty(),
        "empty NORMAL dropped, not iterated"
    );
}

/// Only triangle lists are ingested: a non-TRIANGLES primitive (here POINTS,
/// `mode: 0`) is skipped rather than misread as a triangle list. The retained
/// triangle still identifies its original source slot rather than being
/// renumbered after filtering.
#[test]
fn load_skips_non_triangle_primitive_and_preserves_retained_source_slot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("points-primitive.gltf");
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 36, "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }],
            "accessors": [{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0,0,0], "max": [1,1,0] }],
            "meshes": [{ "primitives": [
                { "mode": 0, "attributes": { "POSITION": 0 } },
                { "attributes": { "POSITION": 0 } }
            ] }],
            "nodes": [{ "mesh": 0 }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }"#,
    )
    .unwrap();

    let doc = animsmith_gltf::load(&path).expect("loads");
    let primitives = &doc.assets.meshes[0].primitives;
    assert_eq!(primitives.len(), 1, "POINTS primitive is skipped");
    assert_eq!(primitives[0].source_primitive_index, Some(1));
}

/// A count-0 `inverseBindMatrices` accessor must not crash the loader.
/// This is read in *two* places — the skeleton build (`build_document`)
/// and asset extraction — so the guard has to cover both. `load` skips
/// the empty IBM accessor (bones fall back to `inverse_bind: None`, mesh
/// IBMs stay empty) and never panics.
#[test]
fn load_survives_zero_count_inverse_bind_matrices() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zero-count-ibm.gltf");
    // Valid triangle mesh + a skin whose inverseBindMatrices is a count-0
    // MAT4 accessor.
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 36, "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0,0,0], "max": [1,1,0] },
                { "bufferView": 0, "componentType": 5126, "count": 0, "type": "MAT4" }
            ],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
            "nodes": [{ "name": "joint" }, { "name": "mesh", "mesh": 0, "skin": 0 }],
            "skins": [{ "joints": [0], "inverseBindMatrices": 1 }],
            "scenes": [{ "nodes": [1] }],
            "scene": 0
        }"#,
    )
    .unwrap();

    let doc = animsmith_gltf::load(&path).expect("count-0 IBM parses without panic");
    assert_eq!(
        doc.assets.meshes[0].primitives[0].positions.len(),
        3,
        "mesh still loads"
    );
    assert!(
        doc.assets.instances[0].skin_ibms.is_empty(),
        "empty IBM accessor dropped in asset extraction"
    );
    assert!(
        doc.skeleton.bones.iter().all(|b| b.inverse_bind.is_none()),
        "empty IBM accessor dropped in the skeleton build too"
    );
}

/// A count-0 `indices` accessor must not crash the loader — pins the
/// index-read guard. `load` treats it as an unindexed primitive.
#[test]
fn load_survives_zero_count_indices() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zero-count-indices.gltf");
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 36, "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0,0,0], "max": [1,1,0] },
                { "bufferView": 0, "componentType": 5125, "count": 0, "type": "SCALAR" }
            ],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "indices": 1 }] }],
            "nodes": [{ "mesh": 0 }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }"#,
    )
    .unwrap();

    let doc = animsmith_gltf::load(&path).expect("count-0 indices parses without panic");
    let prim = &doc.assets.meshes[0].primitives[0];
    assert_eq!(prim.positions.len(), 3, "mesh still loads");
    assert!(prim.indices.is_empty(), "empty index accessor dropped");
}

#[test]
fn load_records_secondary_influence_set_sides_without_changing_primary_attributes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secondary-influences.gltf");
    let buffer_path = dir.path().join("secondary-influences.bin");

    let mut bytes = Vec::new();
    for value in [
        [0.0_f32, 0.0, 0.0],
        [1.0_f32, 0.0, 0.0],
        [0.0_f32, 1.0, 0.0],
    ]
    .into_iter()
    .flatten()
    {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    // JOINTS_0, WEIGHTS_0, JOINTS_1, WEIGHTS_2, JOINTS_2.
    bytes.extend_from_slice(&[0, 1, 0, 0].repeat(3));
    for value in [0.75_f32, 0.25, 0.0, 0.0].repeat(3) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&[2, 3, 0, 0].repeat(3));
    for value in [0.1_f32, 0.2, 0.3, 0.4].repeat(3) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&[4, 5, 0, 0].repeat(3));
    std::fs::write(&buffer_path, &bytes).expect("writes buffer");

    std::fs::write(
        &path,
        serde_json::json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "uri": "secondary-influences.bin", "byteLength": bytes.len() }],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
                { "buffer": 0, "byteOffset": 36, "byteLength": 12 },
                { "buffer": 0, "byteOffset": 48, "byteLength": 48 },
                { "buffer": 0, "byteOffset": 96, "byteLength": 12 },
                { "buffer": 0, "byteOffset": 108, "byteLength": 48 },
                { "buffer": 0, "byteOffset": 156, "byteLength": 12 }
            ],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0] },
                { "bufferView": 1, "componentType": 5121, "count": 3, "type": "VEC4" },
                { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4" },
                { "bufferView": 3, "componentType": 5121, "count": 3, "type": "VEC4" },
                { "bufferView": 4, "componentType": 5126, "count": 3, "type": "VEC4" },
                { "bufferView": 5, "componentType": 5121, "count": 3, "type": "VEC4" }
            ],
            "meshes": [{ "primitives": [
                { "attributes": { "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2, "JOINTS_1": 3, "WEIGHTS_2": 4, "JOINTS_2": 5 } },
                { "attributes": { "POSITION": 0 } }
            ] }],
            "nodes": [{ "mesh": 0 }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        })
        .to_string(),
    )
    .expect("writes glTF");

    let doc = animsmith_gltf::load(&path).expect("loads");
    let first = &doc.assets.meshes[0].primitives[0];
    assert_eq!(first.joints, vec![[0, 1, 0, 0]; 3]);
    assert_eq!(first.weights, vec![[0.75, 0.25, 0.0, 0.0]; 3]);
    assert_eq!(
        first.additional_influence_sets,
        vec![
            AdditionalInfluenceSet {
                set_index: 1,
                joints_present: true,
                weights_present: false
            },
            AdditionalInfluenceSet {
                set_index: 2,
                joints_present: true,
                weights_present: true
            },
        ]
    );
    assert!(
        doc.assets.meshes[0].primitives[1]
            .additional_influence_sets
            .is_empty(),
        "a different primitive must not inherit secondary metadata"
    );
}

/// Write a compact glTF fixture with a shared POSITION accessor and optional
/// inverse-bind accessors. `declared_count` is intentionally independent from
/// the readable matrix count so hardening cases can be represented directly.
fn write_source_skeleton_gltf(
    path: &std::path::Path,
    nodes: serde_json::Value,
    skins: serde_json::Value,
    meshes: serde_json::Value,
    inverse_bind_accessors: &[(usize, Vec<Mat4>)],
    scene_roots: &[usize],
) {
    let mut bytes = Vec::new();
    for value in [
        [0.0_f32, 0.0, 0.0],
        [1.0_f32, 0.0, 0.0],
        [0.0_f32, 1.0, 0.0],
    ]
    .into_iter()
    .flatten()
    {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let mut views = vec![serde_json::json!({
        "buffer": 0,
        "byteOffset": 0,
        "byteLength": 36,
    })];
    let mut accessors = vec![serde_json::json!({
        "bufferView": 0,
        "componentType": 5126,
        "count": 3,
        "type": "VEC3",
        "min": [0, 0, 0],
        "max": [1, 1, 0],
    })];
    for (declared_count, matrices) in inverse_bind_accessors {
        let offset = bytes.len();
        for matrix in matrices {
            for value in matrix.to_cols_array() {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let view = views.len();
        // A count-zero accessor is never read by the loader. Reusing the
        // position view keeps the fixture parser-valid without an empty view.
        if *declared_count > 0 {
            views.push(serde_json::json!({
                "buffer": 0,
                "byteOffset": offset,
                "byteLength": matrices.len() * 64,
            }));
        }
        accessors.push(serde_json::json!({
            "bufferView": if *declared_count == 0 { 0 } else { view },
            "componentType": 5126,
            "count": declared_count,
            "type": "MAT4",
        }));
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    std::fs::write(
        path,
        serde_json::json!({
            "asset": { "version": "2.0" },
            "buffers": [{
                "byteLength": bytes.len(),
                "uri": format!("data:application/octet-stream;base64,{encoded}"),
            }],
            "bufferViews": views,
            "accessors": accessors,
            "meshes": meshes,
            "nodes": nodes,
            "skins": skins,
            "scenes": [{ "nodes": scene_roots }],
            "scene": 0,
        })
        .to_string(),
    )
    .expect("writes source-skeleton fixture");
}

#[test]
fn load_preserves_source_node_order_parentage_and_declared_skin_root() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source-order.gltf");
    write_source_skeleton_gltf(
        &path,
        serde_json::json!([
            { "name": "tip", "translation": [0, 1, 0] },
            { "name": "scene-root", "translation": [10, 0, 0], "children": [2, 4] },
            { "name": "rig-helper", "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 1], "children": [3] },
            { "name": "root", "children": [0] },
            { "name": "body", "mesh": 0, "skin": 0 }
        ]),
        serde_json::json!([{ "name": "body-skin", "joints": [3, 0], "skeleton": 2, "inverseBindMatrices": 1 }]),
        serde_json::json!([{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]),
        &[(
            2,
            vec![
                Mat4::IDENTITY,
                Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)),
            ],
        )],
        &[1],
    );

    let doc = animsmith_gltf::load(&path).expect("loads");
    let source = &doc.assets.source_skeleton;
    assert_eq!(source.coverage, SourceSkeletonCoverage::Complete);
    assert_eq!(
        source
            .nodes
            .iter()
            .map(|node| node.source_node_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4],
        "source order is independent from DFS bone order"
    );
    assert_eq!(source.nodes[0].parent_source_node_index, Some(3));
    assert_eq!(source.nodes[2].parent_source_node_index, Some(1));
    assert_eq!(source.nodes[1].scene_root_indices, vec![0]);
    assert!(matches!(
        source.nodes[2].local_rest,
        SourceNodeLocalRest::Matrix(matrix) if matrix.w_axis.x == 1.0
    ));
    assert_eq!(
        doc.skeleton
            .bones
            .iter()
            .map(|bone| bone.name.as_str())
            .collect::<Vec<_>>(),
        vec!["scene-root", "rig-helper", "root", "tip", "body"],
        "normalized skeleton keeps its independent parent-before-child order"
    );

    let skin = &source.skins[0];
    assert_eq!(skin.source_skin_index, 0);
    assert_eq!(skin.name.as_deref(), Some("body-skin"));
    assert_eq!(skin.skeleton_root_source_node_index, Some(2));
    assert_eq!(skin.joint_source_node_indices, vec![3, 0]);
    assert_eq!(
        skin.inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Available
    );
    assert_eq!(skin.inverse_bind_accessor.declared_count, Some(2));
    assert_eq!(skin.inverse_bind_accessor.matrices.len(), 2);
    assert_eq!(
        skin.attachments
            .iter()
            .map(|attachment| (attachment.source_node_index, attachment.source_mesh_index))
            .collect::<Vec<_>>(),
        vec![(4, Some(0))]
    );
}

#[test]
fn load_keeps_multiple_skin_bindings_and_skipped_mesh_attachments_separate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("multiple-skins.gltf");
    write_source_skeleton_gltf(
        &path,
        serde_json::json!([
            { "name": "joint" },
            { "name": "body-a", "mesh": 0, "skin": 0 },
            { "name": "body-b", "mesh": 0, "skin": 1 },
            { "name": "skipped", "mesh": 1, "skin": 0 }
        ]),
        serde_json::json!([
            { "name": "first", "joints": [0], "skeleton": 0, "inverseBindMatrices": 1 },
            { "name": "second", "joints": [0], "inverseBindMatrices": 2 }
        ]),
        serde_json::json!([
            { "primitives": [{ "attributes": { "POSITION": 0 } }] },
            { "primitives": [{ "mode": 0, "attributes": { "POSITION": 0 } }] }
        ]),
        &[
            (
                2,
                vec![
                    Mat4::IDENTITY,
                    Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)),
                ],
            ),
            (1, vec![Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0))]),
        ],
        &[0, 1, 2, 3],
    );

    let doc = animsmith_gltf::load(&path).expect("loads");
    let source = &doc.assets.source_skeleton;
    assert_eq!(source.skins.len(), 2);
    assert_eq!(source.skins[0].joint_source_node_indices, vec![0]);
    assert_eq!(source.skins[1].joint_source_node_indices, vec![0]);
    assert_eq!(source.skins[0].skeleton_root_source_node_index, Some(0));
    assert_eq!(source.skins[1].skeleton_root_source_node_index, None);
    assert_eq!(
        source.skins[0].inverse_bind_accessor.declared_count,
        Some(2)
    );
    assert_eq!(
        source.skins[0].inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Available,
        "extra IBM rows remain valid and visible"
    );
    assert_eq!(source.skins[0].inverse_bind_accessor.matrices.len(), 2);
    assert_eq!(source.skins[1].inverse_bind_accessor.matrices.len(), 1);
    assert_eq!(
        source.skins[0]
            .attachments
            .iter()
            .map(|attachment| (attachment.source_node_index, attachment.source_mesh_index))
            .collect::<Vec<_>>(),
        vec![(1, Some(0)), (3, Some(1))],
        "skin attachment evidence includes a node whose POINTS mesh is skipped"
    );
    assert_eq!(
        source.skins[1]
            .attachments
            .iter()
            .map(|attachment| attachment.source_node_index)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(doc.assets.instances.len(), 2, "POINTS mesh remains skipped");
    assert_eq!(
        doc.skeleton.bones[0].inverse_bind,
        Some(Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0))),
        "legacy bone fallback retains its existing last-skin-wins behavior at its independent topo ordinal"
    );
}

#[test]
fn load_records_absent_empty_short_singular_and_non_finite_inverse_binds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("inverse-bind-states.gltf");
    write_source_skeleton_gltf(
        &path,
        serde_json::json!([
            { "name": "joint" },
            { "mesh": 0, "skin": 0 },
            { "mesh": 0, "skin": 1 },
            { "mesh": 0, "skin": 2 },
            { "mesh": 0, "skin": 3 },
            { "mesh": 0, "skin": 4 }
        ]),
        serde_json::json!([
            { "joints": [0] },
            { "joints": [0], "inverseBindMatrices": 1 },
            { "joints": [0, 0], "inverseBindMatrices": 2 },
            { "joints": [0], "inverseBindMatrices": 3 },
            { "joints": [0], "inverseBindMatrices": 4 }
        ]),
        serde_json::json!([{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]),
        &[
            (0, vec![]),
            (1, vec![Mat4::IDENTITY]),
            (1, vec![Mat4::ZERO]),
            (1, vec![Mat4::from_cols_array(&[f32::NAN; 16])]),
        ],
        &[0, 1, 2, 3, 4, 5],
    );

    let doc = animsmith_gltf::load(&path).expect("loads");
    let skins = &doc.assets.source_skeleton.skins;
    assert_eq!(
        skins[0].inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Absent
    );
    assert_eq!(skins[0].inverse_bind_accessor.declared_count, None);
    assert_eq!(
        skins[1].inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::EmptyAccessor
    );
    assert_eq!(skins[1].inverse_bind_accessor.declared_count, Some(0));
    assert_eq!(
        skins[2].inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::CountMismatch
    );
    assert_eq!(skins[2].inverse_bind_accessor.declared_count, Some(1));
    assert_eq!(
        skins[2].inverse_bind_accessor.matrices,
        vec![Mat4::IDENTITY]
    );
    assert_eq!(
        skins[3].inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Available
    );
    assert_eq!(skins[3].inverse_bind_accessor.matrices, vec![Mat4::ZERO]);
    assert_eq!(
        skins[4].inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Available
    );
    assert!(
        skins[4].inverse_bind_accessor.matrices[0]
            .to_cols_array()
            .iter()
            .all(|value| value.is_nan()),
        "source sidecar retains parseable non-finite matrix values for later coverage reporting"
    );
}

#[test]
fn load_maps_every_source_node_to_the_bone_it_normalized_to() {
    // `SourceNodeAsset::bone` is the only bridge from raw source-node
    // identity to normalized `BoneId`, and `animsmith_core::scale`'s
    // rest/bind planning resolves its caller-declared `source_root_node_index`
    // and every skin joint through it. This fixture deliberately makes source
    // (file) order and bone (DFS) order disagree at four of five nodes, so an
    // identity or off-by-one mapping cannot pass.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source-bone-mapping.gltf");
    write_source_skeleton_gltf(
        &path,
        serde_json::json!([
            { "name": "tip", "translation": [0, 1, 0] },
            { "name": "scene-root", "children": [2, 4] },
            { "name": "rig-helper", "children": [3] },
            { "name": "root", "children": [0] },
            { "name": "body", "mesh": 0, "skin": 0 }
        ]),
        serde_json::json!([{ "joints": [3, 0], "inverseBindMatrices": 1 }]),
        serde_json::json!([{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]),
        &[(
            2,
            vec![
                Mat4::IDENTITY,
                Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)),
            ],
        )],
        &[1],
    );

    let doc = animsmith_gltf::load(&path).expect("loads");
    let source = &doc.assets.source_skeleton;
    // The DFS from the single root visits scene-root, rig-helper, root, tip,
    // body — bone ids 0..4 in that order — while source order stays 0..4 in
    // file order.
    assert_eq!(
        source
            .nodes
            .iter()
            .map(|node| (node.source_node_index, node.bone))
            .collect::<Vec<_>>(),
        vec![
            (0, Some(3)),
            (1, Some(0)),
            (2, Some(1)),
            (3, Some(2)),
            (4, Some(4))
        ],
    );
    // Independent cross-check against the normalized skeleton: the bone each
    // source node claims must be the bone carrying that node's own name.
    for node in &source.nodes {
        let bone = node.bone.expect("every loaded node normalizes to a bone");
        assert_eq!(
            Some(doc.skeleton.bones[bone].name.as_str()),
            node.name.as_deref(),
            "source node {} maps to the wrong bone",
            node.source_node_index
        );
    }
    // The `Option` exists for index alignment, not because a loaded file can
    // contain an unnormalized node: every node the topology DFS fails to
    // reach is a rootless cycle, which is a `LoadError::Topology` instead of
    // a partial load (see `hardening::loader_rejects_cyclic_node_graph`).
    assert!(
        source.nodes.iter().all(|node| node.bone.is_some()),
        "a file that loads at all has no unnormalized source node"
    );
}
