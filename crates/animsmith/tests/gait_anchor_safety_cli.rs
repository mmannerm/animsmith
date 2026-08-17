//! End-to-end gait-anchor safety boundary for the standalone transform CLI.

use animsmith_core::glam::Vec3;
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
                        + Vec3::new(0.0, sign * 0.06 * theta.sin(), 0.0)
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

#[test]
fn transform_refuses_root_motion_before_publishing_output() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let input = dir.path().join("root-motion.glb");
    let output = dir.path().join("anchored.glb");
    animsmith_gltf::write::write(&root_motion_gait(), &input).expect("writes source GLB");
    std::fs::write(
        dir.path().join("animsmith.toml"),
        concat!(
            "[rig]\nprofile = \"auto\"\n\n",
            "[rig.roles]\n",
            "root = \"root\"\n",
            "hips = \"hips\"\n",
            "left_foot = \"left_foot\"\n",
            "right_foot = \"right_foot\"\n",
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
        "horizontal translation 3.0000 m",
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
