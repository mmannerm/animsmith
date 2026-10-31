//! Observable sampling admission and coverage, using self-authored motion.

use animsmith_core::config::{
    CheckSettings, ClipExpectations, GaitGroup, MovementOwner, Pinned, SeveritySetting, SyncGroup,
    TimeComplementSettings,
};
use animsmith_core::evaluation::{CoverageGapCode, EvaluationState, lint_requires_failure};
use animsmith_core::glam::Vec3;
use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::{
    CheckCtx, CheckSelection, Config, MetricGrids, SamplingError, Severity, all_checks,
    evaluate_checks_v2, sample_clip,
};
use std::collections::BTreeSet;

fn document(bones: usize, frames: usize) -> Document {
    Document {
        skeleton: Skeleton {
            bones: (0..bones)
                .map(|index| Bone {
                    name: format!("bone_{index}"),
                    parent: (index > 0).then_some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                })
                .collect(),
        },
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: (0..frames)
                    .map(|i| i as f32 / (frames - 1) as f32)
                    .collect(),
                values: TrackValues::Vec3s(
                    (0..frames)
                        .map(|i| Vec3::X * (2.0 * i as f32 / (frames - 1) as f32))
                        .collect(),
                ),
            }],
        }],
        ..Document::default()
    }
}

#[test]
fn public_sampler_refuses_large_products_and_preserves_analytic_samples() {
    let oversized = document(2048, 4096);
    assert!(matches!(
        sample_clip(&oversized.skeleton, &oversized.clips[0], 4096),
        Err(SamplingError::LimitExceeded { .. })
    ));
    let small = document(1, 3);
    let grid = sample_clip(&small.skeleton, &small.clips[0], 3).unwrap();
    assert_eq!(grid.times, [0.0, 0.5, 1.0]);
    assert_eq!(grid.model_position(1, 0), Vec3::X);
    assert_eq!(grid.model_position(2, 0), Vec3::X * 2.0);
    for request in [0, 1] {
        let grid = sample_clip(&small.skeleton, &small.clips[0], request).unwrap();
        assert_eq!(grid.times, [0.0, 1.0]);
        assert_eq!(grid.model_position(1, 0), Vec3::X * 2.0);
    }
    assert!(sample_clip(&Skeleton::default(), &small.clips[0], usize::MAX).is_err());
}

#[test]
fn refused_sampled_checks_report_missing_work_through_the_public_runner() {
    let mut doc = document(2048, 4096);
    let mut second = doc.clips[0].clone();
    second.name = "run".into();
    doc.clips.push(second);
    let roles = ResolvedRoles::from_names(
        &doc.skeleton,
        [
            (Role::Root, "bone_0".into()),
            (Role::Hips, "bone_1".into()),
            (Role::LeftFoot, "bone_2".into()),
            (Role::RightFoot, "bone_3".into()),
        ],
    );
    let ids = [
        "loop-seam",
        "foot-slide",
        "in-place",
        "root-motion-speed",
        "loop-closure",
        "loop-seam-vel",
        "loop-seam-rot",
        "gait-group",
        "sync-group",
        "time-complement",
    ];
    let mut config = Config::default();
    for id in ids {
        config.checks.insert(
            id.into(),
            CheckSettings {
                severity: Some(SeveritySetting::Warn),
                ..CheckSettings::default()
            },
        );
    }
    config.clips.insert(
        "*".into(),
        ClipExpectations {
            looping: Some(true),
            movement_owner_xz: Some(MovementOwner::Animation),
            speed_mps: Some(Pinned {
                value: 2.0,
                tolerance: 0.1,
            }),
            fps: Some(4095.0),
            ..ClipExpectations::default()
        },
    );
    let members = vec!["walk".into(), "run".into()];
    config.gait_groups.insert(
        "ring".into(),
        GaitGroup {
            clips: members.clone(),
            max_gait_phase_spread: 0.1,
            min_lr_amplitude_m: 0.0,
        },
    );
    config.sync_groups.insert(
        "pair".into(),
        SyncGroup {
            clips: members,
            max_duration_delta_s: 0.0,
            max_frame_count_delta: 0,
            max_fps_delta: 0.0,
            time_complement: Some(TimeComplementSettings {
                min_reflected_time_advantage: 0.1,
                min_lr_amplitude_m: 0.0,
            }),
        },
    );
    let selected: BTreeSet<_> = ids.into_iter().map(str::to_owned).collect();
    let grids = MetricGrids::new(&doc);
    let results = evaluate_checks_v2(
        &CheckCtx::new(&grids, &roles, &config),
        &all_checks(),
        CheckSelection::Only(&selected),
    )
    .unwrap();
    for result in results
        .iter()
        .filter(|result| selected.contains(result.check_id()))
    {
        assert_ne!(
            result.evaluation(),
            EvaluationState::Complete,
            "{}",
            result.check_id()
        );
        assert!(result.findings().is_empty(), "{}", result.check_id());
        let gaps: Vec<_> = result
            .gaps()
            .iter()
            .filter(|gap| {
                gap.code == CoverageGapCode::MEASUREMENT_UNAVAILABLE
                    && gap.message.contains("sampling")
            })
            .collect();
        assert!(
            !gaps.is_empty(),
            "{}: {:?}",
            result.check_id(),
            result.gaps()
        );
        for gap in gaps {
            let scope = gap
                .scope
                .as_ref()
                .expect("refusal identifies its work scope");
            if result.check_id() == "sync-group" {
                // Compatibility also includes independently completed timing
                // comparisons; the existing coarse scope is explicitly partial.
                assert_eq!(result.evaluation(), EvaluationState::Partial);
                assert_eq!(scope.subject.as_deref(), Some("pair"));
                assert!(result.evaluated_scopes().iter().any(|done| done.code
                    == animsmith_core::EvaluationScopeCode::SYNC_MEMBER_MEASUREMENT));
            } else {
                assert!(matches!(scope.subject.as_deref(), Some("walk" | "run")));
                assert!(
                    !result.evaluated_scopes().contains(scope),
                    "{}",
                    result.check_id()
                );
            }
        }
    }
    // Ordinary gaps stay nonblocking for direct embedders; mandatory CLI
    // measurement propagates the error before it can publish an envelope.
    assert!(!lint_requires_failure(
        &results,
        Severity::Error,
        &BTreeSet::new()
    ));
    assert!(animsmith_core::measure::measure_document(&grids, &roles, &config).is_err());
}

#[test]
fn pruning_retains_authored_tracks_when_its_sampling_oracle_is_refused() {
    let mut doc = document(2048, 4096);
    doc.clips[0].tracks[0].values = TrackValues::Vec3s(vec![Vec3::ZERO; 4096]);
    let times = doc.clips[0].tracks[0].times.clone();
    let result =
        animsmith_core::transform::prune_constant_tracks(&doc.skeleton, &mut doc.clips[0], &[]);
    assert!(result.removed.is_empty());
    assert_eq!(result.retained.len(), 1);
    assert_eq!(
        result.retained[0].reason,
        animsmith_core::transform::ConstantTrackRetentionReason::SamplingUnavailable
    );
    assert_eq!(doc.clips[0].tracks.len(), 1);
    assert_eq!(doc.clips[0].tracks[0].times, times);
    let TrackValues::Vec3s(values) = &doc.clips[0].tracks[0].values else {
        panic!("translation storage")
    };
    assert_eq!(values, &vec![Vec3::ZERO; 4096]);
}
