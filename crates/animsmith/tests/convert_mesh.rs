//! Deep validation of full-scene conversion against a real mesh FBX.
//! Gated on ANIMSMITH_MESH_FBX (licensed assets can't ship in-repo):
//!
//! ```console
//! ANIMSMITH_MESH_FBX="/path/to/X Bot.fbx" cargo test -p animsmith --test convert_mesh
//! ```
#![cfg(feature = "fbx")]

#[test]
fn converted_mesh_is_structurally_sound() {
    let Ok(fbx) = std::env::var("ANIMSMITH_MESH_FBX") else {
        eprintln!("skipped: set ANIMSMITH_MESH_FBX to run");
        return;
    };
    let (doc, assets) =
        animsmith_fbx::load_with_assets(std::path::Path::new(&fbx)).expect("FBX loads");
    assert!(!assets.meshes.is_empty(), "fixture must carry meshes");

    let out = std::env::temp_dir().join("animsmith-convert-mesh.glb");
    animsmith_gltf::write::write_with_assets(&doc, &assets, &out).expect("writes");

    let bytes = std::fs::read(&out).unwrap();
    let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");
    let blob = gltf.blob.clone().expect("BIN");
    let get = |b: gltf::Buffer| -> Option<&[u8]> { Some(&blob[..b.length()]) };

    let mut corners = 0usize;
    for mesh in gltf.meshes() {
        for prim in mesh.primitives() {
            let reader = prim.reader(get);
            let positions: Vec<[f32; 3]> = reader.read_positions().expect("POSITION").collect();
            let index_count = reader
                .read_indices()
                .map(|i| i.into_u32().count())
                .unwrap_or(positions.len());
            assert!(index_count.is_multiple_of(3), "triangles");
            assert!(
                positions.len() <= index_count,
                "welding must not add vertices"
            );
            corners += index_count;
            // Mesh-local positions stay in source units under ufbx's
            // AdjustTransforms space conversion (the node/skin
            // matrices carry the unit scale), so only finiteness and a
            // non-degenerate extent are asserted here — world-space
            // correctness is the visual/skinning check's job.
            let mut min = [f32::MAX; 3];
            let mut max = [f32::MIN; 3];
            for p in &positions {
                assert!(p.iter().all(|c| c.is_finite()), "non-finite position");
                for c in 0..3 {
                    min[c] = min[c].min(p[c]);
                    max[c] = max[c].max(p[c]);
                }
            }
            assert!(
                (0..3).any(|c| max[c] - min[c] > 1e-3),
                "degenerate primitive extent"
            );
            if let Some(weights) = reader.read_weights(0) {
                let joints: Vec<[u16; 4]> = reader
                    .read_joints(0)
                    .expect("JOINTS_0")
                    .into_u16()
                    .collect();
                let joint_count = gltf.skins().next().expect("skin").joints().count() as u16;
                for (w, j) in weights.into_f32().zip(joints) {
                    let sum: f32 = w.iter().sum();
                    assert!(
                        (sum - 1.0).abs() < 1e-3 || sum == 0.0,
                        "weights not normalized: {w:?}"
                    );
                    assert!(
                        j.iter().all(|&idx| idx < joint_count),
                        "joint index out of range: {j:?} (count {joint_count})"
                    );
                }
            }
        }
    }
    assert!(
        corners > 1000,
        "expected a real mesh, got {corners} corners"
    );

    // Every skin's IBM count matches its joint count.
    for skin in gltf.skins() {
        let ibms = skin
            .reader(get)
            .read_inverse_bind_matrices()
            .expect("IBMs")
            .count();
        assert_eq!(ibms, skin.joints().count());
    }
    println!(
        "validated {corners} corners across {} meshes",
        gltf.meshes().count()
    );
}
