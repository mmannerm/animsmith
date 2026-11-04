//! End-to-end gait-anchor safety boundary for the standalone transform CLI.

use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use std::f64::consts::TAU;
use std::process::Command;

const FPS: f32 = 32.0;
const KEYS: usize = 32;

fn timed_vec3_track(bone: usize, values: Vec<Vec3>) -> Track {
    Track {
        bone,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: (0..KEYS).map(|key| key as f32 / FPS).collect(),
        values: TrackValues::Vec3s(values),
    }
}

fn root_motion_gait() -> Document {
    let skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "hips".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::Y,
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "left_foot".into(),
                parent: Some(1),
                rest: Transform {
                    translation: Vec3::new(0.1, -1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "right_foot".into(),
                parent: Some(1),
                rest: Transform {
                    translation: Vec3::new(-0.1, -1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ],
    };
    let foot = |bone: usize, sign: f32| {
        timed_vec3_track(
            bone,
            (0..KEYS)
                .map(|key| {
                    let theta = (TAU * key as f64 / KEYS as f64) as f32;
                    skeleton.bones[bone].rest.translation
                        + Vec3::new(0.0, sign * 0.06 * theta.sin(), sign * 0.06 * theta.sin())
                })
                .collect(),
        )
    };
    let tracks = vec![
        timed_vec3_track(
            0,
            (0..KEYS)
                .map(|key| Vec3::new(key as f32 * 0.1, 0.0, 0.0))
                .collect(),
        ),
        foot(2, 1.0),
        foot(3, -1.0),
    ];
    Document {
        skeleton,
        clips: vec![Clip {
            name: "walk_root_motion".into(),
            duration_s: f64::from((KEYS - 1) as f32 / FPS),
            tracks,
        }],
        ..Document::default()
    }
}

fn use_vertical_local_z_basis(document: &mut Document) {
    document.skeleton.bones[0].rest.rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
}

#[test]
fn transform_refuses_root_motion_before_publishing_output() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let input = dir.path().join("root-motion.glb");
    let output = dir.path().join("anchored.glb");
    let mut source = root_motion_gait();
    use_vertical_local_z_basis(&mut source);
    animsmith_gltf::write::write(&source, &input).expect("writes source GLB");
    std::fs::write(
        dir.path().join("animsmith.toml"),
        concat!(
            "[rig]\nprofile = \"auto\"\n\n",
            "[rig.roles]\n",
            "root = \"root\"\n",
            "hips = \"hips\"\n",
            "left_foot = \"left_foot\"\n",
            "right_foot = \"right_foot\"\n",
            "[checks.loop-closure]\nmax_position_delta_m = 0.02\n",
        ),
    )
    .expect("writes role config");
    std::fs::write(&output, b"existing output").expect("writes output sentinel");

    let result = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir.path())
        .args([
            "--config",
            "animsmith.toml",
            "transform",
            "root-motion.glb",
            "-o",
            "anchored.glb",
            "--clip",
            "walk_root_motion",
            "--gait-anchor",
            "--fps",
            "32",
        ])
        .output()
        .expect("runs animsmith transform");

    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty(), "stdout: {:?}", result.stdout);
    let stderr = String::from_utf8(result.stderr).expect("UTF-8 diagnostic");
    for fact in [
        "walk_root_motion",
        "selected Root bone \"root\"",
        "clip \"walk_root_motion\": cannot gait-anchor",
        "horizontal translation 3.1000 m",
        "yaw 0.000 deg",
        "retain source root motion",
        "runtime phase offsets",
        "trajectory-preserving operation",
    ] {
        assert!(stderr.contains(fact), "missing {fact:?} in: {stderr}");
    }
    assert_eq!(
        std::fs::read(&output).expect("reads unchanged output"),
        b"existing output"
    );
}

#[test]
fn transform_accepts_in_place_gait_when_local_z_is_vertical() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let input = dir.path().join("vertical-basis.glb");
    let output = dir.path().join("anchored.glb");
    let mut source = root_motion_gait();
    use_vertical_local_z_basis(&mut source);
    let TrackValues::Vec3s(root) = &mut source.clips[0].tracks[0].values else {
        unreachable!()
    };
    root.fill(Vec3::ZERO);
    animsmith_gltf::write::write(&source, &input).expect("writes vertical-basis source GLB");
    std::fs::write(
        dir.path().join("animsmith.toml"),
        concat!(
            "[rig]\nprofile = \"auto\"\n\n",
            "[rig.roles]\n",
            "root = \"root\"\n",
            "hips = \"hips\"\n",
            "left_foot = \"left_foot\"\n",
            "right_foot = \"right_foot\"\n",
            "[checks.loop-closure]\nmax_position_delta_m = 0.02\n",
        ),
    )
    .expect("writes role config");

    let result = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir.path())
        .args([
            "--config",
            "animsmith.toml",
            "transform",
            "vertical-basis.glb",
            "-o",
            "anchored.glb",
            "--clip",
            "walk_root_motion",
            "--gait-anchor",
            "--fps",
            "32",
        ])
        .output()
        .expect("runs animsmith transform");

    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8(result.stdout).expect("UTF-8 summary");
    assert!(
        stdout.contains("gait-anchored 'walk_root_motion'"),
        "stdout: {stdout}"
    );
    animsmith_gltf::load(&output).expect("loads published gait-anchored GLB");
}

#[test]
fn multi_clip_stationary_refusal_names_the_failing_clip_and_is_atomic() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let input = dir.path().join("multi.glb");
    let output = dir.path().join("anchored.glb");
    let mut document = root_motion_gait();
    let mut valid = document.clips[0].clone();
    valid.name = "valid_cycle".into();
    let TrackValues::Vec3s(root) = &mut valid.tracks[0].values else {
        unreachable!()
    };
    root.fill(Vec3::ZERO);
    let mut stationary = valid.clone();
    stationary.name = "stationary_cycle".into();
    for track in &mut stationary.tracks[1..] {
        let TrackValues::Vec3s(values) = &mut track.values else {
            unreachable!()
        };
        let first = values[0];
        values.fill(first);
    }
    document.clips = vec![valid, stationary];
    animsmith_gltf::write::write(&document, &input).expect("writes multi-clip GLB");
    std::fs::write(
        dir.path().join("animsmith.toml"),
        concat!(
            "[rig]\nprofile = \"auto\"\n\n",
            "[rig.roles]\n",
            "root = \"root\"\n",
            "hips = \"hips\"\n",
            "left_foot = \"left_foot\"\n",
            "right_foot = \"right_foot\"\n",
            "[checks.loop-closure]\nmax_position_delta_m = 0.02\n",
        ),
    )
    .expect("writes role config");
    std::fs::write(&output, b"existing output").expect("writes output sentinel");

    let result = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir.path())
        .args([
            "--config",
            "animsmith.toml",
            "transform",
            "multi.glb",
            "-o",
            "anchored.glb",
            "--gait-anchor",
            "--fps",
            "32",
        ])
        .output()
        .expect("runs multi-clip transform");

    assert_eq!(result.status.code(), Some(2));
    assert!(
        result.stdout.is_empty(),
        "later refusal must suppress earlier clip success output: {:?}",
        result.stdout
    );
    let stderr = String::from_utf8(result.stderr).expect("UTF-8 diagnostic");
    assert!(
        stderr.contains("clip \"stationary_cycle\": no usable stride anchor"),
        "stderr: {stderr}"
    );
    assert_eq!(std::fs::read(output).unwrap(), b"existing output");
}

#[test]
fn source_head_seam_refuses_transform_and_assembly_before_publication() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.glb");
    let mut source = root_motion_gait();
    source.clips[0].name = "walk".into();
    let TrackValues::Vec3s(root) = &mut source.clips[0].tracks[0].values else {
        unreachable!()
    };
    root.fill(Vec3::ZERO);
    for (track, sign) in source.clips[0].tracks[1..].iter_mut().zip([1.0, -1.0]) {
        let TrackValues::Vec3s(values) = &mut track.values else {
            unreachable!()
        };
        for (key, value) in values.iter_mut().enumerate() {
            *value = Vec3::new(
                sign * 0.1,
                -1.0 + sign
                    * 0.025
                    * (std::f32::consts::TAU * key as f32 / (KEYS - 1) as f32).sin(),
                0.0,
            );
        }
    }
    source.skeleton.bones.push(Bone {
        name: "head".into(),
        parent: Some(1),
        rest: Transform::IDENTITY,
        inverse_bind: None,
    });
    source.clips[0].tracks.push(Track {
        bone: 4,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: (0..KEYS).map(|key| key as f32 / FPS).collect(),
        values: TrackValues::Quats(
            (0..KEYS)
                .map(|key| {
                    Quat::from_rotation_y((5.0 * key as f32 / (KEYS - 1) as f32).to_radians())
                })
                .collect(),
        ),
    });
    std::fs::write(dir.path().join("animsmith.toml"), concat!(
        "[rig.roles]\nroot=\"root\"\nhips=\"hips\"\nleft_foot=\"left_foot\"\nright_foot=\"right_foot\"\n",
        "[clips.walk]\nloop=true\n",
    )).unwrap();
    animsmith_gltf::write::write(&source, &input).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_animsmith"))
            .current_dir(dir.path())
            .args(["--config", "animsmith.toml"])
            .args(args)
            .output()
            .unwrap()
    };
    let lint = run(&[
        "lint",
        "source.glb",
        "--select",
        "loop-closure",
        "--format",
        "json",
    ]);
    assert_eq!(lint.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&lint.stdout).unwrap();
    let findings = report["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["check_id"] == "loop-closure")
        .unwrap()["findings"]
        .as_array()
        .unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["bone"], "head");
    assert_eq!(findings[0]["clip"], "walk");

    let operations = vec![vec![
        "transform",
        "source.glb",
        "-o",
        "anchored.glb",
        "--gait-anchor",
        "--fps",
        "32",
    ]];
    #[cfg(feature = "fbx")]
    let operations = {
        let mut operations = operations;
        std::fs::write(dir.path().join("recipe.toml"), concat!(
            "schema_version=7\nschema=\"urn:animsmith:schema:character-assembly-recipe:7\"\n",
            "input_root=\".\"\nbase_input=\"source.glb\"\ncomplete_tracks=false\ncanonicalize_skin=false\nground_and_center=false\nfps=32.0\n",
            "[[clips]]\nname=\"walk\"\ninput=\"source.glb\"\ntake=\"walk\"\ngait_anchor=true\n",
        )).unwrap();
        operations.push(vec![
            "assemble",
            "recipe.toml",
            "-o",
            "anchored.glb",
            "--evidence",
            "evidence.json",
        ]);
        operations
    };
    let source_bytes = std::fs::read(&input).unwrap();
    for args in &operations {
        std::fs::write(dir.path().join("anchored.glb"), b"prior artifact").unwrap();
        std::fs::write(dir.path().join("evidence.json"), b"prior evidence").unwrap();
        let refusal = run(args);
        assert_eq!(
            refusal.status.code(),
            Some(if args[0] == "assemble" { 1 } else { 2 })
        );
        assert!(refusal.stdout.is_empty());
        let error = String::from_utf8(refusal.stderr).unwrap();
        for expected in [
            "walk",
            "head",
            "source wrap at 0.968750 s",
            "loop-closure",
            "5.000000",
        ] {
            assert!(
                error.contains(expected),
                "{args:?}: missing {expected:?}: {error}"
            );
        }
        assert_eq!(
            std::fs::read(dir.path().join("anchored.glb")).unwrap(),
            b"prior artifact"
        );
        assert_eq!(
            std::fs::read(dir.path().join("evidence.json")).unwrap(),
            b"prior evidence"
        );
        assert_eq!(std::fs::read(&input).unwrap(), source_bytes);
    }
    // Change only the bad head channel. Both real producers must now publish.
    let TrackValues::Quats(head) = &mut source.clips[0].tracks.last_mut().unwrap().values else {
        unreachable!()
    };
    head.fill(Quat::IDENTITY);
    animsmith_gltf::write::write(&source, &input).unwrap();
    for args in &operations {
        std::fs::write(dir.path().join("anchored.glb"), b"prior artifact").unwrap();
        let success = run(args);
        assert!(
            success.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&success.stderr)
        );
        animsmith_gltf::load(&dir.path().join("anchored.glb")).unwrap();
    }
}
