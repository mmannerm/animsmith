//! Public gait-phase availability boundaries on analytic clips.

use animsmith_core::glam::Vec3;
use animsmith_core::measure::{MeasurementAvailability, measure_document};
use animsmith_core::metrics::{MIN_STRIDE_STEP_M, foot_cycle_metrics};
use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::{Config, MetricGrids};

fn translation_track(bone: usize, values: Vec<Vec3>) -> Track {
    let times = match values.len() {
        2 => vec![0.0, 1.0],
        3 => vec![0.0, 0.5, 1.0],
        5 => vec![0.0, 0.25, 0.5, 0.75, 1.0],
        count => panic!("unsupported analytic key count {count}"),
    };
    Track {
        bone,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times,
        values: TrackValues::Vec3s(values),
    }
}

fn analytic_document() -> (Document, ResolvedRoles) {
    let skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "hips".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "left_foot".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(-0.1, -1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "right_foot".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(0.1, -1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ],
    };
    let roles = ResolvedRoles::from_names(
        &skeleton,
        [
            (Role::Hips, "hips".into()),
            (Role::LeftFoot, "left_foot".into()),
            (Role::RightFoot, "right_foot".into()),
        ],
    );
    let clips = vec![
        Clip {
            name: "stationary".into(),
            duration_s: 1.0,
            tracks: vec![translation_track(0, vec![Vec3::ZERO; 3])],
        },
        Clip {
            name: "common_mode".into(),
            duration_s: 1.0,
            tracks: vec![
                translation_track(1, vec![Vec3::ZERO, Vec3::Y * 0.2, Vec3::ZERO]),
                translation_track(2, vec![Vec3::ZERO, Vec3::Y * 0.2, Vec3::ZERO]),
            ],
        },
        Clip {
            name: "constant_offset".into(),
            duration_s: 1.0,
            tracks: vec![
                translation_track(1, vec![Vec3::Y * 0.1; 3]),
                translation_track(2, vec![Vec3::ZERO; 3]),
            ],
        },
        Clip {
            name: "alternating_in_place".into(),
            duration_s: 1.0,
            tracks: vec![
                translation_track(1, vec![Vec3::ZERO, Vec3::Y * 1.0e-4, Vec3::ZERO]),
                translation_track(2, vec![Vec3::ZERO, Vec3::Y * -1.0e-4, Vec3::ZERO]),
            ],
        },
        Clip {
            name: "minimum_positive".into(),
            duration_s: 1.0,
            tracks: vec![
                translation_track(
                    1,
                    vec![
                        Vec3::ZERO,
                        Vec3::new(0.0, f32::from_bits(1), 0.0),
                        Vec3::ZERO,
                    ],
                ),
                translation_track(2, vec![Vec3::ZERO; 3]),
            ],
        },
        Clip {
            name: "higher_harmonic".into(),
            duration_s: 1.0,
            tracks: vec![
                translation_track(
                    1,
                    vec![
                        Vec3::ZERO,
                        Vec3::Y * 0.1,
                        Vec3::ZERO,
                        Vec3::Y * 0.1,
                        Vec3::ZERO,
                    ],
                ),
                translation_track(2, vec![Vec3::ZERO; 5]),
            ],
        },
        Clip {
            name: "too_short".into(),
            duration_s: 1.0,
            tracks: vec![translation_track(0, vec![Vec3::ZERO; 2])],
        },
    ];
    (
        Document {
            skeleton,
            clips,
            ..Document::default()
        },
        roles,
    )
}

#[test]
fn public_measurements_distinguish_no_phase_subject_from_derivation_failure() {
    let (document, roles) = analytic_document();
    let grids = MetricGrids::new(&document);

    for name in ["stationary", "common_mode", "constant_offset"] {
        let clip_index = document
            .clips
            .iter()
            .position(|clip| clip.name == name)
            .expect("named flat clip");
        let grid = grids.grid(clip_index).expect("analytic metric grid");
        let raw = foot_cycle_metrics(&grid, &roles, MIN_STRIDE_STEP_M)
            .expect("flat bilateral cycle remains measurable");
        assert_eq!(raw.lr_amplitude_m, 0.0);
        assert_eq!(
            raw.gait_phase, None,
            "the raw metric boundary must not retain an arbitrary phase for a flat signal"
        );
    }
    for name in [
        "alternating_in_place",
        "minimum_positive",
        "higher_harmonic",
    ] {
        let clip_index = document
            .clips
            .iter()
            .position(|clip| clip.name == name)
            .expect("named positive-swing clip");
        let raw = foot_cycle_metrics(
            &grids.grid(clip_index).expect("positive-swing metric grid"),
            &roles,
            MIN_STRIDE_STEP_M,
        )
        .expect("positive-swing bilateral cycle");
        assert!(raw.lr_amplitude_m > 0.0, "{name}");
        assert!(
            raw.gait_phase.is_some(),
            "{name}: every representable positive swing retains the fitted raw phase"
        );
    }

    let config: Config = serde_json::from_value(serde_json::json!({
        "gait_groups": { "consumer_only": {
            "clips": ["minimum_positive", "higher_harmonic"],
            "max_gait_phase_spread": 0.1,
            "min_lr_amplitude_m": 1.0
        }}
    }))
    .expect("consumer gait confidence policy");
    let measured = measure_document(&grids, &roles, &config);

    for name in ["stationary", "common_mode", "constant_offset"] {
        let clip = &measured[name];
        assert_eq!(clip.gait_availability, MeasurementAvailability::Measured);
        let gait = clip.gait.as_ref().expect("parent gait remains measured");
        assert_eq!(gait.lr_amplitude_m, 0.0);
        assert_eq!(gait.phase, None);
        assert_eq!(
            gait.phase_availability,
            MeasurementAvailability::NotApplicable,
            "{name} has no L-R oscillation and therefore no phase subject"
        );
    }

    for name in [
        "alternating_in_place",
        "minimum_positive",
        "higher_harmonic",
    ] {
        let gait = measured[name]
            .gait
            .as_ref()
            .expect("positive foot alternation remains a measured gait");
        assert!(gait.lr_amplitude_m > 0.0, "{name}");
        assert!(gait.phase.is_some(), "{name}");
        assert_eq!(
            gait.phase_availability,
            MeasurementAvailability::Measured,
            "{name}: positive evidence remains measured despite a larger consumer confidence floor"
        );
    }

    let too_short = &measured["too_short"];
    assert!(too_short.gait.is_none());
    assert_eq!(
        too_short.gait_availability,
        MeasurementAvailability::Unavailable,
        "a failed applicable cycle is a parent derivation failure, not nested phase non-applicability"
    );
}
