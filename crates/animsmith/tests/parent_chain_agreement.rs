//! Over-rejection sweep for the source-projection/skeleton parent-chain
//! agreement `animsmith_core::scale` validates at every public entry point.
//!
//! The check is only sound if it refuses *contradictory* documents and
//! nothing else, and the failure mode that matters is the quiet one: a
//! producer whose ordinary output stops planning. `animsmith-core`'s own unit
//! tests cover hand-built documents; this covers the real producers, on real
//! bytes, including the reloaded round-trip a published artifact is re-proved
//! through.
//!
//! Every document below is asserted to *plan*, not merely to validate: the
//! validation is private, and a whole-document conversion at factor `1.0` is
//! the cheapest public call that runs it. The operation is deliberately the
//! whole-document one — the check is a document-shape fact every entry point
//! gets, and a rest/bind plan would refuse most of these fixtures for
//! unrelated reasons.

use animsmith_core::glam;
use animsmith_core::model::{Document, SourceSkeletonCoverage};
use animsmith_core::scale::{
    ScaleCapabilityCoverage, ScaleCapabilityFacts, ScaleOperation, ScaleRequest, plan_scale,
};
use animsmith_core::{
    SkinnedBindPoseCanonicalizationOptions, SkinnedBindPosePlacement, bake_static_mesh_transforms,
    canonicalize_skinned_bind_pose,
};
use std::path::{Path, PathBuf};

fn complete_capability() -> ScaleCapabilityFacts {
    // `ScaleCapabilityFacts` is `#[non_exhaustive]`, so it is built from its
    // default and assigned into rather than named field by field.
    let mut capability = ScaleCapabilityFacts::default();
    capability.coverage = ScaleCapabilityCoverage::Complete;
    capability
}

/// Assert `document` still plans, and report which producer it came from when
/// it does not.
fn accepts(label: &str, document: &Document) {
    let capability = complete_capability();
    if let Err(error) = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.0 },
        document,
        capability: &capability,
    }) {
        panic!("{label} is newly refused by document-shape validation: {error}");
    }
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn load_gltf_bytes(label: &str, name: &str, bytes: &[u8]) -> Document {
    animsmith_gltf::load_bytes(Path::new(name), bytes)
        .unwrap_or_else(|error| panic!("{label} loads: {error}"))
}

#[test]
fn the_gltf_loader_never_produces_a_disagreeing_parent_chain() {
    // The loader is the only in-tree producer that declares `Complete`
    // source-skeleton coverage, so it is the only one the check actually
    // examines — every assertion here is a live one rather than a coverage
    // gate short-circuit.
    let fixtures: Vec<(&str, Vec<u8>)> = vec![
        (
            "testkit rest_bind_scale_rig_glb",
            animsmith_testkit::rest_bind_scale_rig_glb(),
        ),
        (
            "testkit rest_bind_scale_rig_gltf",
            animsmith_testkit::rest_bind_scale_rig_gltf(),
        ),
        (
            "testkit unaffected_bind_scale_rig_glb",
            animsmith_testkit::unaffected_bind_scale_rig_glb(),
        ),
        (
            "testkit nodes_only_scale_rig_glb",
            animsmith_testkit::nodes_only_scale_rig_glb(),
        ),
        (
            "testkit oversized_proof_scale_rig_glb",
            animsmith_testkit::oversized_proof_scale_rig_glb(),
        ),
        (
            "crates/animsmith/testdata/rig.gltf",
            std::fs::read(repo_path("crates/animsmith/testdata/rig.gltf")).expect("rig fixture"),
        ),
        (
            "examples/assets/walk.glb",
            std::fs::read(repo_path("examples/assets/walk.glb")).expect("walk fixture"),
        ),
        (
            "examples/assets/clip.glb",
            std::fs::read(repo_path("examples/assets/clip.glb")).expect("clip fixture"),
        ),
        (
            "examples/assets/walk-dirty.glb",
            std::fs::read(repo_path("examples/assets/walk-dirty.glb")).expect("dirty fixture"),
        ),
    ];
    for (label, bytes) in &fixtures {
        let name = if label.ends_with(".gltf") {
            "sweep.gltf"
        } else {
            "sweep.glb"
        };
        let document = load_gltf_bytes(label, name, bytes);
        assert_eq!(
            document.assets.source_skeleton.coverage,
            SourceSkeletonCoverage::Complete,
            "{label} declares complete coverage, so the check is not skipped"
        );
        accepts(label, &document);
    }
}

#[test]
fn a_written_and_reloaded_gltf_document_still_plans() {
    // Publication goes out through the writer and comes back through the
    // loader, and it is the *reloaded* document a downstream re-proof sees.
    // The writer does not carry `source_skeleton` across, so the reloaded
    // projection is derived afresh from whatever node order the writer chose.
    let document = load_gltf_bytes(
        "testkit rest_bind_scale_rig_glb",
        "sweep.glb",
        &animsmith_testkit::rest_bind_scale_rig_glb(),
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("roundtrip.glb");
    animsmith_gltf::write::write(&document, &path).expect("the loaded rig writes");
    let written = std::fs::read(&path).expect("the written artifact reads back");
    let reloaded = load_gltf_bytes("written round-trip", "roundtrip.glb", &written);
    assert_eq!(
        reloaded.assets.source_skeleton.coverage,
        SourceSkeletonCoverage::Complete
    );
    accepts("written round-trip", &reloaded);
}

#[test]
fn the_synthesizing_producers_carry_no_projection_to_disagree_with() {
    // `static_bake` and `skinned_canonical` build their own rig and emit
    // `SourceSkeletonAssets::default()` — an empty projection under
    // `Unavailable` coverage. That is not a document whose two chains
    // disagree; it is a document with one chain.
    //
    // Note what these assertions do *not* rest on. They are not the coverage
    // gate's justification: an empty projection is accepted with the gate open
    // too, since injectivity and parent preservation quantify over nothing and
    // downward closure is vacuous when no bone has a projecting node. Verified
    // by deleting the gate, and pinned in `animsmith_core` by
    // `an_empty_projection_under_complete_coverage_is_accepted`. What these
    // assertions catch is a check that grew a surjectivity requirement, which
    // would refuse every document below.
    let mut source = load_gltf_bytes(
        "testkit rest_bind_scale_rig_glb",
        "sweep.glb",
        &animsmith_testkit::rest_bind_scale_rig_glb(),
    );
    // Canonicalization refuses an animated rig; the projection is what this
    // sweep is about, and it is the same either way.
    source.clips.clear();

    let canonical = canonicalize_skinned_bind_pose(
        &source,
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: glam::Mat4::IDENTITY,
            placement: SkinnedBindPosePlacement::Preserve,
        },
    )
    .expect("the rig canonicalizes");
    assert_eq!(
        canonical.document.assets.source_skeleton.coverage,
        SourceSkeletonCoverage::Unavailable
    );
    assert!(canonical.document.assets.source_skeleton.nodes.is_empty());
    assert!(!canonical.document.skeleton.bones.is_empty());
    accepts("skinned_canonical output", &canonical.document);

    let baked = bake_static_mesh_transforms(&static_bake_input()).expect("the fixture bakes");
    assert_eq!(
        baked.document.assets.source_skeleton.coverage,
        SourceSkeletonCoverage::Unavailable
    );
    assert!(baked.document.assets.source_skeleton.nodes.is_empty());
    assert!(!baked.document.skeleton.bones.is_empty());
    accepts("static_bake output", &baked.document);
}

/// A minimal unskinned mesh attachment: the shape `bake_static_mesh_transforms`
/// consumes, which no skinned in-tree glTF fixture supplies.
fn static_bake_input() -> Document {
    use animsmith_core::glam::Vec3;
    use animsmith_core::model::{
        MeshAsset, MeshInstance, Primitive, SceneAsset, SceneAssets, Skeleton,
    };
    use animsmith_core::{Bone, Transform};

    Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "prop".into(),
                parent: None,
                rest: Transform {
                    translation: Vec3::new(1.0, 2.0, 3.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            }],
        },
        clips: Vec::new(),
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "triangle".into(),
                source_mesh_index: 0,
                primitives: vec![Primitive {
                    indices: vec![0, 1, 2],
                    positions: vec![
                        Vec3::new(0.0, 0.0, 0.0),
                        Vec3::new(1.0, 0.0, 0.0),
                        Vec3::new(0.0, 1.0, 0.0),
                    ],
                    normals: vec![Vec3::Z; 3],
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
            scenes: vec![SceneAsset {
                source_scene_index: 0,
                name: None,
                roots: vec![0],
            }],
            default_scene: Some(0),
            ..SceneAssets::default()
        },
        source: Default::default(),
    }
}

#[test]
fn the_testkit_document_builder_still_plans() {
    let document = animsmith_testkit::two_bone_rotation_doc(
        "idle",
        animsmith_testkit::quats_from_angles(&[0.0, 0.1, 0.2, 0.3, 0.4]),
        true,
    );
    accepts("testkit two_bone_rotation_doc", &document);
}

#[cfg(feature = "fbx")]
#[test]
fn the_fbx_loader_still_plans() {
    // The FBX loader sets no `source_skeleton` at all, so its documents carry
    // the `Unavailable` default and the check does not examine them. Asserted
    // rather than assumed: a later loader that started projecting source nodes
    // would land here first.
    for name in ["rigged_triangle.fbx", "rigged_triangle_empty_take.fbx"] {
        let path = repo_path(&format!("crates/animsmith-fbx/testdata/{name}"));
        let document = animsmith_fbx::load(&path).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            document.assets.source_skeleton.coverage,
            SourceSkeletonCoverage::Unavailable
        );
        accepts(name, &document);
    }
}
