//! Test-only binary64 conformance oracle for sampled-local-trs-blend-v1.
//!
//! Expected values come from the same f32 MetricGrids samples embedded in
//! the HTML, widened before interpolation. Nothing here enters user reports.

use crate::{ReportInputs, render};
use animsmith_core::glam::{DMat4, DQuat, DVec3, Quat, Vec3};
use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use animsmith_core::{Config, MetricGrids, PoseGrid, ResolvedRoles};
use base64::Engine as _;
use serde_json::{Value, json};

const PHASES: [f64; 3] = [0.0, 0.375, 1.0];
const WEIGHTS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

fn frame_at_phase(grid: &PoseGrid, phase: f64) -> usize {
    (phase.clamp(0.0, 1.0) * (grid.frame_count() - 1) as f64).round() as usize
}

/// Deliberately uses glam's binary64 quaternion and affine matrix operations,
/// independently of the browser's scalar implementation. Endpoints bypass FK.
fn reference(
    skeleton: &Skeleton,
    a: &PoseGrid,
    b: &PoseGrid,
    phase: f64,
    weight: f64,
) -> Vec<[f64; 3]> {
    assert!((0.0..=1.0).contains(&weight));
    assert!(!skeleton.bones.is_empty() && skeleton.bones.len() <= 1024);
    let fa = frame_at_phase(a, phase);
    let fb = frame_at_phase(b, phase);
    if weight == 0.0 || weight == 1.0 {
        let (grid, frame) = if weight == 0.0 { (a, fa) } else { (b, fb) };
        return (0..skeleton.bones.len())
            .map(|bone| grid.model_position(frame, bone).as_dvec3().to_array())
            .collect();
    }
    let mut worlds: Vec<DMat4> = Vec::with_capacity(skeleton.bones.len());
    for (bone, node) in skeleton.bones.iter().enumerate() {
        let la = a.local(fa, bone);
        let lb = b.local(fb, bone);
        let qa = DQuat::from_array(la.rotation.to_array().map(f64::from)).normalize();
        let mut qb = DQuat::from_array(lb.rotation.to_array().map(f64::from)).normalize();
        if qa.dot(qb) < 0.0 {
            qb = -qb;
        }
        let rotation = (qa * (1.0 - weight) + qb * weight).normalize();
        let translation =
            la.translation.as_dvec3() * (1.0 - weight) + lb.translation.as_dvec3() * weight;
        let scale = la.scale.as_dvec3() * (1.0 - weight) + lb.scale.as_dvec3() * weight;
        let local = DMat4::from_scale_rotation_translation(scale, rotation, translation);
        let world = match node.parent {
            Some(parent) => {
                assert!(parent < bone);
                worlds[parent] * local
            }
            None => local,
        };
        assert!(world.is_finite());
        assert!(
            world
                .to_cols_array()
                .iter()
                .all(|v| (*v as f32).is_finite())
        );
        worlds.push(world);
    }
    worlds
        .iter()
        .map(|world| world.w_axis.truncate().to_array())
        .collect()
}

fn bone(parent: Option<usize>, rest: Transform) -> Bone {
    Bone {
        name: parent.map_or_else(|| "root".into(), |p| format!("child-{p}")),
        parent,
        rest,
        inverse_bind: None,
    }
}

fn track(bone: usize, property: Property, values: TrackValues, duration: f32) -> Track {
    let frames = values.len();
    Track {
        bone,
        property,
        // Exact sampled signs, including antipodal and zero-dot test cases,
        // must survive sampling; temporal interpolation is not under test.
        interpolation: Interpolation::Step,
        times: (0..frames)
            .map(|i| duration * i as f32 / (frames - 1) as f32)
            .collect(),
        values,
    }
}

fn clip(name: &str, locals: &[Transform], frames: usize, duration: f32) -> Clip {
    let mut tracks = Vec::new();
    for (bone, local) in locals.iter().enumerate() {
        tracks.push(track(
            bone,
            Property::Translation,
            TrackValues::Vec3s(vec![local.translation; frames]),
            duration,
        ));
        tracks.push(track(
            bone,
            Property::Rotation,
            TrackValues::Quats(vec![local.rotation; frames]),
            duration,
        ));
        tracks.push(track(
            bone,
            Property::Scale,
            TrackValues::Vec3s(vec![local.scale; frames]),
            duration,
        ));
    }
    Clip {
        name: name.into(),
        duration_s: f64::from(duration),
        tracks,
    }
}

fn document(a: Vec<Transform>, b: Vec<Transform>) -> Document {
    assert_eq!(a.len(), b.len());
    Document {
        skeleton: Skeleton {
            bones: a
                .iter()
                .enumerate()
                .map(|(i, local)| bone(i.checked_sub(1), *local))
                .collect(),
        },
        clips: vec![clip("primary", &a, 3, 1.0), clip("paired", &b, 5, 2.0)],
        ..Document::default()
    }
}

fn rotating_parent() -> Document {
    let child = Transform {
        translation: Vec3::X,
        ..Transform::IDENTITY
    };
    document(
        vec![Transform::IDENTITY, child],
        vec![
            Transform {
                // Exact zero dot against identity: the +Z sign is retained.
                rotation: Quat::from_xyzw(0.0, 0.0, 1.0, 0.0),
                ..Transform::IDENTITY
            },
            child,
        ],
    )
}

fn fixtures() -> Vec<(&'static str, Document)> {
    let mut sparse = document(
        vec![
            Transform {
                translation: Vec3::new(2.0, -3.0, 4.0),
                rotation: Quat::from_rotation_z(0.7),
                scale: Vec3::new(1.5, 0.75, 2.0),
            },
            Transform {
                translation: Vec3::new(0.1, 0.2, -0.3),
                rotation: Quat::from_rotation_x(-0.4),
                scale: Vec3::new(0.7, 1.2, 0.9),
            },
        ],
        vec![Transform::IDENTITY; 2],
    );
    // Only root translation is authored: every rotation, scale and child
    // channel must come from the nontrivial rest transform through MetricGrids.
    sparse.clips[0].tracks = vec![track(
        0,
        Property::Translation,
        TrackValues::Vec3s(vec![Vec3::new(2.0, -3.0, 4.0), Vec3::ONE, Vec3::ZERO]),
        1.0,
    )];
    sparse.clips[1].tracks = vec![track(
        0,
        Property::Translation,
        TrackValues::Vec3s((0..5).map(|i| Vec3::new(i as f32, 2.0, -1.0)).collect()),
        2.0,
    )];

    let mut quaternion_a = vec![Transform::IDENTITY; 5];
    for local in &mut quaternion_a[1..] {
        local.translation = Vec3::new(1.0, 0.25, -0.5);
    }
    quaternion_a[0].rotation = Quat::from_rotation_y(0.8);
    let mut quaternion_b = quaternion_a.clone();
    quaternion_b[0].rotation = -quaternion_a[0].rotation;
    quaternion_b[1].rotation = Quat::from_rotation_x(0.0002);
    quaternion_b[2].rotation = Quat::from_xyzw(0.0, 0.0, -1.0, 0.0);
    // Admitted nonunit quaternions must be normalized before mixing.
    quaternion_a[3].rotation = Quat::from_rotation_y(0.3) * 1.00005;
    quaternion_b[3].rotation = Quat::from_rotation_x(-0.9) * 0.99995;

    let affine_a = vec![
        Transform {
            translation: Vec3::new(10.0, -4.0, 3.0),
            rotation: Quat::from_rotation_y(0.8),
            scale: Vec3::new(2.0, 1.0, 0.5),
        },
        Transform {
            translation: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::from_rotation_z(0.6),
            scale: Vec3::new(0.5, 2.0, 1.0),
        },
        Transform {
            translation: Vec3::new(0.7, -1.1, 0.2),
            rotation: Quat::from_rotation_x(-0.4),
            ..Transform::IDENTITY
        },
        Transform {
            translation: Vec3::ONE,
            ..Transform::IDENTITY
        },
    ];
    let mut affine_b = affine_a.clone();
    affine_b[0].translation = Vec3::new(-2.0, 5.0, 0.1);
    affine_b[0].rotation = Quat::from_rotation_z(-0.3);
    affine_b[0].scale = Vec3::new(-2.0, 3.0, 0.0);
    affine_b[1].rotation = Quat::from_rotation_x(1.1);
    affine_b[1].scale = Vec3::new(1.5, -2.0, 0.5);

    let deep = |magnitude: f32| {
        let a: Vec<_> = (0..96)
            .map(|i| Transform {
                translation: Vec3::new(magnitude, magnitude * -0.25, magnitude * 0.125),
                rotation: Quat::from_rotation_z(0.017 * (i % 5) as f32),
                scale: Vec3::new(1.002, 0.999, 1.001),
            })
            .collect();
        let b: Vec<_> = (0..96)
            .map(|i| Transform {
                translation: Vec3::new(magnitude * -0.3, magnitude * 0.5, magnitude),
                rotation: Quat::from_rotation_y(-0.023 * (i % 7) as f32),
                scale: Vec3::new(0.997, 1.003, 0.999),
            })
            .collect();
        document(a, b)
    };
    vec![
        ("rotating-parent", rotating_parent()),
        ("missing-channels-unequal-grids", sparse),
        (
            "quaternion-hemispheres",
            document(quaternion_a, quaternion_b),
        ),
        ("affine-signed-scales", document(affine_a, affine_b)),
        ("deep-small", deep(0.000001)),
        ("deep-large", deep(1_000_000.0)),
    ]
}

#[test]
fn rotating_parent_has_analytic_arc_and_signed_zero_dot_tie() {
    let mut doc = rotating_parent();
    for sign in [1.0, -1.0] {
        doc.clips[1].tracks[1].values =
            TrackValues::Quats(vec![Quat::from_xyzw(0.0, 0.0, sign, 0.0); 5]);
        let grids = MetricGrids::new(&doc);
        let a = grids.grid(0).unwrap();
        let b = grids.grid(1).unwrap();
        let child = DVec3::from_array(reference(&doc.skeleton, &a, &b, 0.375, 0.5)[1]);
        assert!((child - DVec3::new(0.0, f64::from(sign), 0.0)).length() < 1e-12);
        let wrong = (a.model_position(1, 1) + b.model_position(2, 1)).as_dvec3() * 0.5;
        assert!(
            (child - wrong).length() > 0.99,
            "model-position lerp must fail"
        );
    }
}

#[test]
fn affine_oracle_preserves_shear_and_signed_scale() {
    // A stretched root followed by a rotated child is an affine composition.
    // Collapsing world rotation/scale separately would put the tip at Y * 2.
    let locals = vec![
        Transform {
            translation: Vec3::new(3.0, -2.0, 1.0),
            scale: Vec3::new(2.0, 1.0, 1.0),
            ..Transform::IDENTITY
        },
        Transform {
            rotation: Quat::from_xyzw(0.0, 0.0, 1.0, 1.0).normalize(),
            ..Transform::IDENTITY
        },
        Transform {
            translation: Vec3::X,
            ..Transform::IDENTITY
        },
    ];
    let doc = document(locals.clone(), locals);
    let grids = MetricGrids::new(&doc);
    let actual = reference(
        &doc.skeleton,
        &grids.grid(0).unwrap(),
        &grids.grid(1).unwrap(),
        0.0,
        0.5,
    );
    assert!((DVec3::from_array(actual[2]) - DVec3::new(3.0, -1.0, 1.0)).length() < 1e-12);

    let child = Transform {
        translation: Vec3::X,
        ..Transform::IDENTITY
    };
    let doc = document(
        vec![Transform::IDENTITY, child],
        vec![
            Transform {
                scale: -Vec3::ONE,
                ..Transform::IDENTITY
            },
            child,
        ],
    );
    let grids = MetricGrids::new(&doc);
    for (weight, x) in [(0.25, 0.5), (0.5, 0.0), (0.75, -0.5)] {
        let actual = reference(
            &doc.skeleton,
            &grids.grid(0).unwrap(),
            &grids.grid(1).unwrap(),
            1.0,
            weight,
        );
        assert_eq!(actual[1], [x, 0.0, 0.0]);
    }
}

#[test]
fn sampled_defaults_and_independent_phase_mapping_are_preserved() {
    let (_, doc) = fixtures().remove(1);
    let grids = MetricGrids::new(&doc);
    let a = grids.grid(0).unwrap();
    let b = grids.grid(1).unwrap();
    assert_eq!((a.frame_count(), b.frame_count()), (3, 5));
    assert_eq!(
        (frame_at_phase(&a, 0.375), frame_at_phase(&b, 0.375)),
        (1, 2)
    );
    assert_eq!((a.times[1], b.times[2]), (0.5, 1.0));
    for grid in [&a, &b] {
        for frame in 0..grid.frame_count() {
            assert_eq!(grid.local(frame, 1), doc.skeleton.bones[1].rest);
            assert_eq!(
                grid.local(frame, 0).rotation,
                doc.skeleton.bones[0].rest.rotation
            );
            assert_eq!(grid.local(frame, 0).scale, doc.skeleton.bones[0].rest.scale);
        }
    }
}

fn assert_embedded_samples(html: &str, grids: [&PoseGrid; 2]) {
    let marker = r#"<script type="application/json" id="report-data">"#;
    let raw = html
        .split_once(marker)
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0;
    let data: Value = serde_json::from_str(raw).unwrap();
    for (index, grid) in grids.into_iter().enumerate() {
        let encoded = data["clips"][index]["locals"].as_str().unwrap_or_else(|| {
            panic!(
                "golden clip must pass blend admission: {}",
                data["clips"][index]["blend_omission"]
            )
        });
        let local_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let position_bytes = base64::engine::general_purpose::STANDARD
            .decode(data["clips"][index]["positions"].as_str().unwrap())
            .unwrap();
        let mut expected_locals = Vec::new();
        let mut expected_positions = Vec::new();
        for frame in 0..grid.frame_count() {
            for bone in 0..grid.bone_count() {
                let local = grid.local(frame, bone);
                for value in local
                    .translation
                    .to_array()
                    .into_iter()
                    .chain(local.rotation.to_array())
                    .chain(local.scale.to_array())
                {
                    expected_locals.extend_from_slice(&value.to_le_bytes());
                }
                for value in grid.model_position(frame, bone).to_array() {
                    expected_positions.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        assert_eq!(
            local_bytes, expected_locals,
            "oracle and browser must read identical local bytes"
        );
        assert_eq!(
            position_bytes, expected_positions,
            "endpoint oracle must read identical source bytes"
        );
    }
}

/// The regular Rust test run exercises every expected pose. The viewer gate
/// opts into writing fixtures with ANIMSMITH_BLEND_GOLDENS, avoiding a golden
/// payload in production HTML or a checked-in generated corpus.
#[test]
fn write_blend_goldens() {
    let output = std::env::var_os("ANIMSMITH_BLEND_GOLDENS").map(std::path::PathBuf::from);
    if let Some(path) = &output {
        std::fs::create_dir_all(path).unwrap();
    }
    let mut cases = Vec::new();
    for (name, doc) in fixtures() {
        let grids = MetricGrids::new(&doc);
        let a = grids.grid(0).expect("primary clip must have a metric grid");
        let b = grids.grid(1).expect("paired clip must have a metric grid");
        let mut samples: Vec<Value> = Vec::new();
        for phase in PHASES {
            for weight in WEIGHTS {
                let expected = reference(&doc.skeleton, &a, &b, phase, weight);
                assert_eq!(expected.len(), doc.skeleton.bones.len());
                for (bone, position) in expected.iter().enumerate() {
                    assert!(
                        position
                            .iter()
                            .all(|v| v.is_finite() && (*v as f32).is_finite())
                    );
                    if weight == 0.0 || weight == 1.0 {
                        let grid = if weight == 0.0 { &a } else { &b };
                        assert_eq!(
                            *position,
                            grid.model_position(frame_at_phase(grid, phase), bone)
                                .as_dvec3()
                                .to_array()
                        );
                    }
                }
                let fa = frame_at_phase(&a, phase);
                let fb = frame_at_phase(&b, phase);
                samples.push(json!({
                    "phase": phase, "weight": weight, "expected": expected,
                    "frame_a": fa, "frame_b": fb, "time_a": a.times[fa], "time_b": b.times[fb],
                }));
            }
        }
        let html_name = format!("{name}.html");
        let mut names = vec![(animsmith_core::profile::Role::Root, "root".to_owned())];
        if doc.skeleton.bones.len() >= 4 {
            names.extend([
                (
                    animsmith_core::profile::Role::Hips,
                    doc.skeleton.bones[1].name.clone(),
                ),
                (
                    animsmith_core::profile::Role::LeftFoot,
                    doc.skeleton.bones[2].name.clone(),
                ),
                (
                    animsmith_core::profile::Role::RightFoot,
                    doc.skeleton.bones[3].name.clone(),
                ),
            ]);
        }
        let roles = ResolvedRoles::from_names(&doc.skeleton, names);
        let config = Config::default();
        let html = render(ReportInputs::new(&grids, &roles, &[], &config));
        assert_embedded_samples(&html, [&a, &b]);
        if let Some(path) = &output {
            std::fs::write(path.join(&html_name), html).unwrap();
        }
        cases.push(json!({"name": name, "html": html_name, "samples": samples}));
    }
    if let Some(path) = &output {
        std::fs::write(
            path.join("blend-goldens.json"),
            serde_json::to_vec_pretty(&json!({"cases": cases})).unwrap(),
        )
        .unwrap();
    }
}
