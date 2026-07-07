//! Mesh/skin/material emission: a synthetic skinned triangle carried in
//! `Document::assets` must parse as valid glTF with every attribute
//! readable and weights normalized.

use animsmith_core::model::*;
use glam::{Mat4, Quat, Vec3};

/// A valid 1×1 white PNG.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xA7, 0x35, 0x81, 0x84, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

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
            node: 0,
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
                };
                prim.weld();
                prim
            }],
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
        }],
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
    animsmith_gltf::write::write(&doc, &path).expect("writes");

    let bytes = std::fs::read(&path).unwrap();
    let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");
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
    assert_eq!(mesh.skin_joints, vec![0, 1]);
    assert_eq!(
        mesh.skin_ibms,
        vec![
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)),
        ],
    );

    // The skinned mesh hangs off the writer's dedicated holder node —
    // a synthesized 3rd bone — not the original bone-0 node it was
    // authored on. This pins the `MeshAsset::node` round-trip claim.
    assert_eq!(doc.skeleton.bones.len(), 3, "2 skeleton bones + holder");
    assert_eq!(mesh.node, 2, "mesh maps to the holder bone, not bone 0");

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
                node: 0,
                primitives: vec![Primitive {
                    material: None,
                    indices: vec![],
                    positions: positions.clone(),
                    normals: normals.clone(),
                    uvs: uvs.clone(),
                    joints: joints.clone(),
                    weights: weights.clone(),
                }],
                skin_joints: vec![],
                skin_ibms: vec![],
            }],
            materials: vec![],
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
        doc.assets.meshes[0].skin_joints,
        vec![1, 0],
        "joints remapped to bone ids, both loader paths agreeing"
    );
    // The mesh hangs off node 1 (root), which is bone 0 — `MeshAsset::node`
    // is likewise a bone id, not the raw node index.
    assert_eq!(
        doc.assets.meshes[0].node, 0,
        "MeshAsset::node is the remapped bone id"
    );
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

/// A base-color texture whose image is an external sibling file — the
/// `Source::Uri` external-file branch of `read_image`, read relative to
/// the glTF via the same containment rule as external buffers.
#[test]
fn load_reads_external_file_texture() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("tex.png"), TINY_PNG).unwrap();
    let path = dir.path().join("external-image.gltf");
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "images": [{ "uri": "tex.png", "mimeType": "image/png" }],
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

/// Only triangle lists are ingested: a non-TRIANGLES primitive (here
/// POINTS, `mode: 0`) is skipped rather than misread as a triangle list,
/// so it never silently round-trips as corrupted TRIANGLES.
#[test]
fn load_skips_non_triangle_primitive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("points-primitive.gltf");
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 36, "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }],
            "accessors": [{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0,0,0], "max": [1,1,0] }],
            "meshes": [{ "primitives": [{ "mode": 0, "attributes": { "POSITION": 0 } }] }],
            "nodes": [{ "mesh": 0 }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }"#,
    )
    .unwrap();

    let doc = animsmith_gltf::load(&path).expect("loads");
    assert!(
        doc.assets.meshes.is_empty(),
        "POINTS primitive skipped, not ingested as triangles"
    );
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
        doc.assets.meshes[0].skin_ibms.is_empty(),
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
