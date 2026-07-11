//! CI-run validation of full-scene conversion against a self-authored
//! ASCII FBX fixture carrying one skinned triangle and one clip.
#![cfg(feature = "fbx")]

use std::path::{Path, PathBuf};

const RIGGED_TRIANGLE_FBX: &str = include_str!("../../animsmith-fbx/testdata/rigged_triangle.fbx");
const RIGGED_TRIANGLE_WITH_EMPTY_TAKE_FBX: &str =
    include_str!("../../animsmith-fbx/testdata/rigged_triangle_empty_take.fbx");

fn unique_temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("animsmith-{name}-"))
        .tempdir()
        .unwrap()
}

fn write_fixture(dir: &Path) -> PathBuf {
    let fbx = dir.join("rigged_triangle.fbx");
    std::fs::write(&fbx, RIGGED_TRIANGLE_FBX).expect("writes FBX fixture");
    fbx
}

fn write_fixture_with_empty_take(dir: &Path) -> PathBuf {
    let fbx = dir.join("rigged_triangle_with_empty_take.fbx");
    std::fs::write(&fbx, RIGGED_TRIANGLE_WITH_EMPTY_TAKE_FBX)
        .expect("writes FBX fixture with empty take");
    fbx
}

fn mesh_count(glb: &Path) -> usize {
    let bytes = std::fs::read(glb).unwrap();
    gltf::Gltf::from_slice(&bytes)
        .expect("valid glTF")
        .meshes()
        .count()
}

fn loaded_meshes(glb: &Path) -> Vec<animsmith_core::model::MeshAsset> {
    animsmith_gltf::load(glb)
        .expect("converted GLB loads")
        .assets
        .meshes
}

fn baseline_meshes(fbx: &Path, out: &Path) -> Vec<animsmith_core::model::MeshAsset> {
    let doc = animsmith_fbx::load(fbx).expect("FBX loads");
    animsmith_gltf::write::write(&doc, out).expect("writes baseline GLB");
    loaded_meshes(out)
}

fn assert_meshes_match(
    expected: &[animsmith_core::model::MeshAsset],
    actual: &[animsmith_core::model::MeshAsset],
) {
    assert_eq!(actual.len(), expected.len(), "mesh count");
    for (expected, actual) in expected.iter().zip(actual) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.node, expected.node);
        assert_eq!(actual.skin_joints, expected.skin_joints);
        assert_eq!(actual.skin_ibms, expected.skin_ibms);
        assert_eq!(actual.primitives.len(), expected.primitives.len());
        for (expected, actual) in expected.primitives.iter().zip(&actual.primitives) {
            assert_eq!(actual.material, expected.material);
            assert_eq!(actual.indices, expected.indices);
            assert_eq!(actual.positions, expected.positions);
            assert_eq!(actual.normals, expected.normals);
            assert_eq!(actual.uvs, expected.uvs);
            assert_eq!(actual.joints, expected.joints);
            assert_eq!(actual.weights, expected.weights);
        }
    }
}

#[test]
fn converted_mesh_is_structurally_sound() {
    let dir = unique_temp_dir("convert-mesh");
    let fbx = write_fixture(dir.path());
    let doc = animsmith_fbx::load(&fbx).expect("FBX loads");
    assert!(!doc.assets.meshes.is_empty(), "fixture must carry meshes");
    assert_eq!(doc.assets.meshes[0].skin_joints.len(), 1);

    let out = dir.path().join("converted.glb");
    animsmith_gltf::write::write(&doc, &out).expect("writes");

    let bytes = std::fs::read(&out).unwrap();
    let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");
    assert_eq!(gltf.skins().count(), 1, "skin survives conversion");
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
            let weights = reader.read_weights(0).expect("WEIGHTS_0");
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
    assert_eq!(corners, 3, "fixture is a single triangle");

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

/// End-to-end through the real `convert` subcommand: the default run
/// carries geometry, and `--animation-only` strips it — the uniform
/// behaviour #33 promises, exercised at the CLI contract (not just the
/// library round-trip).
#[test]
fn cli_convert_carries_and_strips_geometry() {
    let dir = unique_temp_dir("cli-convert");
    let fbx = write_fixture(dir.path());

    let convert = |out: &Path, animation_only: bool| {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"));
        cmd.arg("convert").arg(&fbx).arg("-o").arg(out);
        if animation_only {
            cmd.arg("--animation-only");
        }
        let output = cmd.output().expect("runs animsmith");
        assert!(
            output.status.success(),
            "convert exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
        let written = animsmith_gltf::load(out).expect("loads converted output");
        let corners: usize = written
            .assets
            .meshes
            .iter()
            .flat_map(|mesh| mesh.primitives.iter())
            .map(|primitive| primitive.positions.len())
            .sum();
        let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
        assert_eq!(
            stdout,
            format!(
                "wrote {} ({} node(s), {} clip(s), {} mesh(es) / {} position(s), {} material(s))\n",
                out.display(),
                written.skeleton.bones.len(),
                written.clips.len(),
                written.assets.meshes.len(),
                corners,
                written.assets.materials.len(),
            ),
            "a fully serialized FBX document keeps the ordinary summary shape"
        );
    };

    let carried = dir.path().join("carried.glb");
    convert(&carried, false);
    let baseline = baseline_meshes(&fbx, &dir.path().join("baseline.glb"));
    let carried_meshes = loaded_meshes(&carried);
    assert_meshes_match(&baseline, &carried_meshes);

    let stripped = dir.path().join("stripped.glb");
    convert(&stripped, true);
    assert_eq!(
        mesh_count(&stripped),
        0,
        "convert --animation-only strips geometry"
    );
}

/// The convert summary describes the serialized glTF, not the FBX loader's
/// intermediate document. Empty FBX takes are retained by the loader but have
/// no glTF channels, so the writer deliberately omits them.
#[test]
fn cli_convert_summary_reports_written_artifact_and_dropped_empty_take() {
    let dir = unique_temp_dir("cli-convert-summary");
    let fbx = write_fixture_with_empty_take(dir.path());
    let source = animsmith_fbx::load(&fbx).expect("FBX with empty take loads");
    assert_eq!(
        source
            .clips
            .iter()
            .map(|clip| (clip.name.as_str(), clip.tracks.is_empty()))
            .collect::<Vec<_>>(),
        vec![("take", false), ("empty", true)],
        "the added empty take is distinct from the animated take"
    );

    let out = dir.path().join("converted.glb");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("convert")
        .arg(&fbx)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("runs animsmith");
    assert!(output.status.success(), "convert exited {}", output.status);

    let written = animsmith_gltf::load(&out).expect("converted GLB loads");
    assert_eq!(
        (
            source
                .clips
                .iter()
                .map(|clip| clip.name.as_str())
                .collect::<Vec<_>>(),
            written
                .clips
                .iter()
                .map(|clip| clip.name.as_str())
                .collect::<Vec<_>>(),
        ),
        (vec!["take", "empty"], vec!["take"]),
        "the empty take is the source clip omitted from the artifact"
    );
    let written_corners: usize = written
        .assets
        .meshes
        .iter()
        .flat_map(|mesh| mesh.primitives.iter())
        .map(|primitive| primitive.positions.len())
        .sum();
    let dropped_clips = source.clips.len() - written.clips.len();
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert_eq!(
        stdout,
        format!(
            "wrote {} ({} node(s), {} clip(s), {} mesh(es) / {} position(s), {} material(s)); dropped {} clip(s) with no writable tracks\n",
            out.display(),
            written.skeleton.bones.len(),
            written.clips.len(),
            written.assets.meshes.len(),
            written_corners,
            written.assets.materials.len(),
            dropped_clips,
        ),
        "the CLI summary matches the written artifact"
    );
}

/// End-to-end through the real `transform` subcommand: a transform pass
/// must carry the input's geometry to the output (the data-loss bug #33
/// fixes).
#[test]
fn cli_transform_preserves_geometry() {
    let dir = unique_temp_dir("cli-transform");
    let fbx = write_fixture(dir.path());
    let doc = animsmith_fbx::load(&fbx).expect("FBX loads");
    assert!(
        !doc.clips.is_empty(),
        "fixture carries a transformable clip"
    );

    let out = dir.path().join("transformed.glb");

    // A hold-extend is a real (non-no-op) transform; the geometry must
    // survive it.
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("transform")
        .arg(&fbx)
        .arg("-o")
        .arg(&out)
        .arg("--hold-extend")
        .arg("0.25")
        .status()
        .expect("runs animsmith");
    assert!(status.success(), "transform exited {status}");

    let meshes = mesh_count(&out);
    assert!(meshes > 0, "transform carries geometry to its output");
    let baseline = baseline_meshes(&fbx, &dir.path().join("baseline.glb"));
    let transformed_meshes = loaded_meshes(&out);
    assert_meshes_match(&baseline, &transformed_meshes);
}
