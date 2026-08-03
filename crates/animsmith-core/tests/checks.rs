//! Mutation-style check tests, after the incubating pipeline's
//! discipline: start from a clean document (zero findings), corrupt
//! exactly one thing, and assert exactly the expected check fires.

use animsmith_core::config::{CheckSettings, ClipExpectations, Pinned};
use animsmith_core::model::*;
use animsmith_core::profile::ResolvedRoles;
use animsmith_core::{
    CheckCtx, CheckSelection, Config, MetricGrids, Severity, SeveritySetting, evaluate_checks,
    mechanical_checks,
};
use glam::{Quat, Vec3};

/// A clean 2-bone document with a rotation and a translation track.
fn clean_doc() -> Document {
    let skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "hips".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "spine".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(0.0, 0.3, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ],
    };
    let rotation = Track {
        bone: 1,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Quats(vec![
            Quat::IDENTITY,
            Quat::from_rotation_y(0.8),
            Quat::from_rotation_y(1.6),
        ]),
    };
    let translation = Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Vec3s(vec![
            Vec3::ZERO,
            Vec3::new(0.0, 0.05, 0.5),
            Vec3::new(0.0, 0.0, 1.0),
        ]),
    };
    Document {
        skeleton,
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![rotation, translation],
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}

fn lint(doc: &Document) -> Vec<animsmith_core::Finding> {
    lint_with_config(doc, &Config::default())
}

fn lint_with_config(doc: &Document, config: &Config) -> Vec<animsmith_core::Finding> {
    evaluations_with_config(doc, config)
        .into_iter()
        .flat_map(|check| check.findings().to_vec())
        .collect()
}

fn evaluations_with_config(
    doc: &Document,
    config: &Config,
) -> Vec<animsmith_core::CheckEvaluation> {
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(doc);
    let ctx = CheckCtx::new(&grids, &roles, config);
    evaluate_checks(&ctx, &mechanical_checks(), CheckSelection::All)
        .expect("valid built-in catalog")
}

fn assert_single(doc: &Document, check_id: &str, severity: Severity) {
    let findings = lint(doc);
    assert_eq!(
        findings.len(),
        1,
        "expected exactly one finding, got: {findings:#?}"
    );
    assert_eq!(findings[0].check_id, check_id);
    assert_eq!(findings[0].severity, severity);
}

#[test]
fn clean_document_has_no_findings() {
    let findings = lint(&clean_doc());
    assert!(findings.is_empty(), "clean doc flagged: {findings:#?}");
}

#[test]
fn nan_value_is_flagged() {
    let mut doc = clean_doc();
    if let TrackValues::Vec3s(v) = &mut doc.clips[0].tracks[1].values {
        v[1].y = f32::NAN;
    }
    assert_single(&doc, "nan", Severity::Error);
}

#[test]
fn nan_time_is_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].tracks[1].times[1] = f32::INFINITY;
    // The corrupted time also breaks monotonicity; the nan finding must
    // be among the results and attributed to the right bone.
    let findings = lint(&doc);
    let nan = findings
        .iter()
        .find(|f| f.check_id == "nan")
        .expect("nan finding");
    assert_eq!(nan.bone.as_deref(), Some("hips"));
}

#[test]
fn non_monotonic_times_are_flagged() {
    let mut doc = clean_doc();
    // Corrupt ordering mid-track, keeping start (0.0) and end (1.0)
    // intact so no other check fires.
    doc.clips[0].tracks[0].times = vec![0.0, 0.6, 0.4, 1.0];
    if let TrackValues::Quats(v) = &mut doc.clips[0].tracks[0].values {
        v.push(Quat::from_rotation_y(2.0));
    }
    assert_single(&doc, "time-monotonic", Severity::Error);
}

#[test]
fn negative_time_beyond_tolerance_is_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].tracks[0].times = vec![-0.01, 0.5, 1.0];
    assert_single(&doc, "time-monotonic", Severity::Error);
}

#[test]
fn f32_quantization_dust_at_zero_is_tolerated() {
    // Bake pipelines that slice frame ranges leave first keys like
    // -1e-6 s; engines clamp these harmlessly.
    let mut doc = clean_doc();
    doc.clips[0].tracks[0].times = vec![-1e-6, 0.5, 1.0];
    let findings = lint(&doc);
    assert!(findings.is_empty(), "dust flagged: {findings:#?}");
}

#[test]
fn late_first_key_is_noted() {
    let mut doc = clean_doc();
    doc.clips[0].tracks[0].times = vec![0.4, 0.7, 1.0];
    let findings = lint(&doc);
    assert!(
        findings
            .iter()
            .any(|f| f.check_id == "time-monotonic" && f.severity == Severity::Note),
        "got: {findings:#?}"
    );
}

#[test]
fn denormalized_quat_is_flagged() {
    let mut doc = clean_doc();
    if let TrackValues::Quats(v) = &mut doc.clips[0].tracks[0].values {
        v[1] = Quat::from_xyzw(0.0, 0.65, 0.0, 0.65); // |q| ≈ 0.92
    }
    let findings = lint(&doc);
    let f = findings
        .iter()
        .find(|f| f.check_id == "quat-norm")
        .expect("quat-norm finding");
    assert_eq!(f.severity, Severity::Error);
    assert_eq!(f.bone.as_deref(), Some("spine"));
}

#[test]
fn hemisphere_flip_is_flagged() {
    let mut doc = clean_doc();
    if let TrackValues::Quats(v) = &mut doc.clips[0].tracks[0].values {
        v[1] = -v[1]; // same rotation, opposite hemisphere
    }
    assert_single(&doc, "quat-flip", Severity::Warning);
}

#[test]
fn zero_duration_is_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].duration_s = 0.0;
    // Track end times no longer matter for a degenerate clip.
    let findings = lint(&doc);
    assert!(
        findings
            .iter()
            .any(|f| f.check_id == "duration-sanity" && f.severity == Severity::Error),
        "got: {findings:#?}"
    );
}

#[test]
fn degenerate_duration_with_a_pin_reports_measured_and_expected_seconds() {
    let mut doc = clean_doc();
    doc.clips[0].duration_s = 0.0;
    let findings = lint_with_config(&doc, &duration_pin(1.0, 0.02));
    let finding = findings
        .iter()
        .find(|finding| finding.check_id == "duration-sanity")
        .expect("degenerate duration finding");
    let value = serde_json::to_value(finding).expect("serializes finding");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(value["measured"], 0.0);
    assert_eq!(value["expected"], 1.0);
}

fn duration_pin(value: f64, tolerance: f64) -> Config {
    let mut config = Config::default();
    config.clips.insert(
        "walk".into(),
        ClipExpectations {
            duration_s: Some(Pinned { value, tolerance }),
            ..Default::default()
        },
    );
    config
}

#[test]
fn duration_pin_tolerance_is_inclusive_in_both_directions() {
    let config = duration_pin(1.0, 0.25);
    for duration_s in [0.75, 1.25] {
        let mut doc = clean_doc();
        doc.clips[0].duration_s = duration_s;
        let findings = lint_with_config(&doc, &config);
        assert!(
            !findings
                .iter()
                .any(|finding| finding.check_id == "duration-sanity"),
            "boundary duration {duration_s} was rejected: {findings:#?}"
        );
    }

    for duration_s in [
        f64::from_bits(0.75f64.to_bits() - 1),
        f64::from_bits(1.25f64.to_bits() + 1),
    ] {
        let mut doc = clean_doc();
        doc.clips[0].duration_s = duration_s;
        let findings = lint_with_config(&doc, &config);
        let duration_findings: Vec<_> = findings
            .iter()
            .filter(|finding| finding.check_id == "duration-sanity")
            .collect();
        assert_eq!(
            duration_findings.len(),
            1,
            "next representable duration outside the boundary must fail: {findings:#?}"
        );
        assert_eq!(duration_findings[0].severity, Severity::Error);
        let value = serde_json::to_value(duration_findings[0]).expect("serializes finding");
        assert_eq!(value["measured"], duration_s);
        assert_eq!(value["expected"], 1.0);
    }
}

#[test]
fn invalid_duration_pins_are_explicit_errors() {
    for pin in [
        Pinned {
            value: f64::NAN,
            tolerance: 0.02,
        },
        Pinned {
            value: 0.0,
            tolerance: 0.02,
        },
        Pinned {
            value: 1.0,
            tolerance: f64::NAN,
        },
        Pinned {
            value: 1.0,
            tolerance: -0.02,
        },
    ] {
        let mut config = Config::default();
        config.clips.insert(
            "walk".into(),
            ClipExpectations {
                duration_s: Some(pin),
                ..Default::default()
            },
        );
        let findings = lint_with_config(&clean_doc(), &config);
        let finding = findings
            .iter()
            .find(|finding| finding.check_id == "duration-sanity")
            .expect("invalid duration pin finding");
        assert_eq!(finding.severity, Severity::Error);
        assert!(finding.message.contains("invalid declared duration pin"));
    }
}

#[test]
fn duration_pin_miss_reports_structured_measured_and_expected_seconds() {
    let doc = clean_doc();
    let findings = lint_with_config(&doc, &duration_pin(1.033, 0.02));
    let finding = findings
        .iter()
        .find(|finding| finding.check_id == "duration-sanity")
        .expect("duration pin finding");
    let value = serde_json::to_value(finding).expect("serializes finding");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(value["clip"], "walk");
    assert_eq!(value["measured"], 1.0);
    assert_eq!(value["expected"], 1.033);
}

#[test]
fn duration_pin_is_still_judged_for_an_empty_clip() {
    let mut doc = clean_doc();
    doc.clips[0].tracks.clear();
    let findings = lint_with_config(&doc, &duration_pin(1.033, 0.02));
    let duration_findings: Vec<_> = findings
        .iter()
        .filter(|finding| finding.check_id == "duration-sanity")
        .collect();
    assert_eq!(
        duration_findings.len(),
        2,
        "empty-track and duration-contract failures should both remain visible: {findings:#?}"
    );
    assert!(duration_findings.iter().any(|finding| {
        finding.severity == Severity::Warning && finding.message == "clip has no tracks"
    }));
    let pin_miss = duration_findings
        .iter()
        .find(|finding| finding.severity == Severity::Error)
        .expect("duration pin error");
    let value = serde_json::to_value(pin_miss).expect("serializes finding");
    assert_eq!(value["measured"], 1.0);
    assert_eq!(value["expected"], 1.033);
}

#[test]
fn mismatched_channel_ends_are_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].tracks[1].times = vec![0.0, 0.3, 0.6]; // rotation still ends at 1.0
    assert_single(&doc, "duration-sanity", Severity::Warning);
}

#[test]
fn single_key_pin_does_not_count_toward_end_spread() {
    // A one-key track at t=0 is a pinned value (a common bake idiom),
    // not a truncated channel.
    let mut doc = clean_doc();
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0],
        values: TrackValues::Vec3s(vec![Vec3::new(0.0, 0.3, 0.0)]),
    });
    let findings = lint(&doc);
    assert!(findings.is_empty(), "pin flagged: {findings:#?}");
}

#[test]
fn empty_clip_is_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].tracks.clear();
    assert_single(&doc, "duration-sanity", Severity::Warning);
}

#[test]
fn scale_keys_are_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::splat(1.2)]),
    });
    assert_single(&doc, "scale-keys", Severity::Warning);
}

#[test]
fn non_uniform_scale_gets_second_finding() {
    let mut doc = clean_doc();
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::new(1.2, 1.0, 1.0)]),
    });
    let findings = lint(&doc);
    assert_eq!(
        findings
            .iter()
            .filter(|f| f.check_id == "scale-keys")
            .count(),
        1,
        "got: {findings:#?}"
    );
    assert_eq!(
        findings
            .iter()
            .filter(|f| f.check_id == "non-uniform-scale")
            .count(),
        1,
        "got: {findings:#?}"
    );
    assert!(findings.iter().any(|finding| {
        finding.check_id == "non-uniform-scale" && finding.severity == Severity::Warning
    }));
}

#[test]
fn all_ones_scale_track_is_constant_not_scaling() {
    let mut doc = clean_doc();
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::ONE]),
    });
    assert_single(&doc, "constant-track", Severity::Note);
}

fn add_scale_track(
    doc: &mut Document,
    interpolation: Interpolation,
    times: Vec<f32>,
    values: Vec<Vec3>,
) {
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Scale,
        interpolation,
        times,
        values: TrackValues::Vec3s(values),
    });
}

fn scale_finding_ids(doc: &Document) -> Vec<&'static str> {
    scale_finding_ids_with_config(doc, &Config::default())
}

fn scale_finding_ids_with_config(doc: &Document, config: &Config) -> Vec<&'static str> {
    let mut ids: Vec<_> = lint_with_config(doc, config)
        .into_iter()
        .filter(|finding| {
            matches!(
                finding.check_id,
                "scale-keys" | "non-uniform-scale" | "constant-nonunit-scale" | "constant-track"
            )
        })
        .map(|finding| finding.check_id)
        .collect();
    ids.sort_unstable();
    ids
}

fn presence_enabled_config_with(severity: SeveritySetting) -> Config {
    let mut config = Config::default();
    config.checks.insert(
        "constant-nonunit-scale".into(),
        CheckSettings {
            severity: Some(severity),
            ..CheckSettings::default()
        },
    );
    config
}

fn presence_enabled_config() -> Config {
    presence_enabled_config_with(SeveritySetting::Note)
}

#[test]
fn constant_uniform_scale_is_redundancy_not_temporal_variation() {
    let mut doc = clean_doc();
    add_scale_track(
        &mut doc,
        Interpolation::Linear,
        vec![0.0, 0.5, 1.0],
        vec![Vec3::ONE; 3],
    );
    assert_eq!(scale_finding_ids(&doc), vec!["constant-track"]);
}

#[test]
fn step_and_linear_scale_changes_are_temporal_variation() {
    for interpolation in [Interpolation::Step, Interpolation::Linear] {
        let mut doc = clean_doc();
        add_scale_track(
            &mut doc,
            interpolation,
            vec![0.0, 1.0],
            vec![Vec3::splat(2.0), Vec3::splat(3.0)],
        );
        assert_eq!(scale_finding_ids(&doc), vec!["scale-keys"]);
        assert_eq!(
            lint(&doc)
                .iter()
                .filter(|finding| finding.check_id == "scale-keys")
                .count(),
            1
        );
    }
}

#[test]
fn cubic_interior_temporal_excursion_is_not_constant() {
    let mut doc = clean_doc();
    // Equal endpoints, but equal positive tangents generate an interior
    // Hermite excursion. A key-only test would incorrectly call this bloat.
    add_scale_track(
        &mut doc,
        Interpolation::CubicSpline,
        vec![0.0, 1.0],
        vec![
            Vec3::ZERO,
            Vec3::ONE,
            Vec3::splat(0.004),
            Vec3::splat(0.004),
            Vec3::ONE,
            Vec3::ZERO,
        ],
    );
    assert_eq!(scale_finding_ids(&doc), vec!["scale-keys"]);
}

#[test]
fn cubic_opposite_tangents_create_a_one_component_excursion() {
    let mut doc = clean_doc();
    add_scale_track(
        &mut doc,
        Interpolation::CubicSpline,
        vec![0.0, 1.0],
        vec![
            Vec3::ZERO,
            Vec3::ONE,
            Vec3::new(0.004, 0.0, 0.0),
            Vec3::new(-0.004, 0.0, 0.0),
            Vec3::ONE,
            Vec3::ZERO,
        ],
    );
    assert_eq!(
        scale_finding_ids(&doc),
        vec!["non-uniform-scale", "scale-keys"]
    );
}

#[test]
fn cubic_y_and_z_only_tangents_are_temporal_variation_and_non_uniform() {
    for tangent in [Vec3::new(0.0, 0.004, 0.0), Vec3::new(0.0, 0.0, 0.004)] {
        let mut doc = clean_doc();
        add_scale_track(
            &mut doc,
            Interpolation::CubicSpline,
            vec![0.0, 1.0],
            vec![
                Vec3::ZERO,
                Vec3::ONE,
                tangent,
                -tangent,
                Vec3::ONE,
                Vec3::ZERO,
            ],
        );
        assert_eq!(
            scale_finding_ids(&doc),
            vec!["non-uniform-scale", "scale-keys"]
        );
    }
}

#[test]
fn cubic_temporal_variation_respects_the_tolerance_direction() {
    // Equal tangents produce a peak-to-trough range of about 0.1924 times the
    // tangent, placing these cases just below and above the 1e-4 threshold.
    let below_tolerance_tangent = 0.0005f32;
    let over_tolerance_tangent = 0.0006f32;
    for (tangent, expected) in [
        (below_tolerance_tangent, false),
        (over_tolerance_tangent, true),
    ] {
        let mut doc = clean_doc();
        add_scale_track(
            &mut doc,
            Interpolation::CubicSpline,
            vec![0.0, 1.0],
            vec![
                Vec3::ZERO,
                Vec3::ONE,
                Vec3::splat(tangent),
                Vec3::splat(tangent),
                Vec3::ONE,
                Vec3::ZERO,
            ],
        );
        assert_eq!(
            scale_finding_ids(&doc).contains(&"scale-keys"),
            expected,
            "tangent {tangent:?}"
        );
    }
}

#[test]
fn cubic_duration_scales_tangent_excursion() {
    // Put the decisive long segment on each side in turn. Both must exceed
    // tolerance after glTF's tangent-times-dt scaling.
    for times in [vec![0.0, 0.25, 2.25], vec![0.0, 2.0, 2.25]] {
        let mut doc = clean_doc();
        add_scale_track(
            &mut doc,
            Interpolation::CubicSpline,
            times,
            vec![
                Vec3::ZERO,
                Vec3::ONE,
                Vec3::splat(0.0003),
                Vec3::splat(0.0003),
                Vec3::ONE,
                Vec3::splat(0.0003),
                Vec3::splat(0.0003),
                Vec3::ONE,
                Vec3::ZERO,
            ],
        );
        assert_eq!(scale_finding_ids(&doc), vec!["scale-keys"]);
    }
}

#[test]
fn cubic_tangents_use_their_own_segment_duration() {
    let scale_values = vec![
        Vec3::ZERO,
        Vec3::ONE,
        Vec3::splat(0.0004),
        Vec3::ZERO,
        Vec3::ONE,
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ONE,
        Vec3::ZERO,
    ];

    let mut short_first = clean_doc();
    add_scale_track(
        &mut short_first,
        Interpolation::CubicSpline,
        vec![0.0, 0.25, 2.25],
        scale_values.clone(),
    );
    assert_eq!(scale_finding_ids(&short_first), vec!["constant-track"]);

    let mut long_first = clean_doc();
    add_scale_track(
        &mut long_first,
        Interpolation::CubicSpline,
        vec![0.0, 2.0, 2.25],
        scale_values,
    );
    assert_eq!(scale_finding_ids(&long_first), vec!["scale-keys"]);
}

#[test]
fn cubic_interior_non_uniformity_has_its_own_finding() {
    let mut doc = clean_doc();
    add_scale_track(
        &mut doc,
        Interpolation::CubicSpline,
        vec![0.0, 1.0],
        vec![
            Vec3::ZERO,
            Vec3::ONE,
            Vec3::new(0.004, 0.0, 0.0),
            Vec3::new(0.004, 0.0, 0.0),
            Vec3::ONE,
            Vec3::ZERO,
        ],
    );
    assert_eq!(
        scale_finding_ids(&doc),
        vec!["non-uniform-scale", "scale-keys"]
    );
}

#[test]
fn cubic_off_grid_interior_extremum_is_detected() {
    let mut doc = clean_doc();
    // With equal endpoints and this tangent pair, the x extremum is at u=1/3,
    // not an endpoint or midpoint.
    add_scale_track(
        &mut doc,
        Interpolation::CubicSpline,
        vec![0.0, 1.0],
        vec![
            Vec3::ZERO,
            Vec3::ONE,
            Vec3::new(0.0008, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::ONE,
            Vec3::ZERO,
        ],
    );
    assert_eq!(
        scale_finding_ids(&doc),
        vec!["non-uniform-scale", "scale-keys"]
    );
}

#[test]
fn constant_nonunit_scale_is_opt_in_and_distinct_from_nonuniformity() {
    let mut doc = clean_doc();
    add_scale_track(
        &mut doc,
        Interpolation::Linear,
        vec![0.0, 1.0],
        vec![Vec3::new(1.2, 1.0, 1.0); 2],
    );
    assert_eq!(
        scale_finding_ids(&doc),
        vec!["constant-track", "non-uniform-scale"]
    );
    assert_eq!(
        scale_finding_ids_with_config(&doc, &presence_enabled_config()),
        vec![
            "constant-nonunit-scale",
            "constant-track",
            "non-uniform-scale"
        ]
    );
    let evaluations = evaluations_with_config(&doc, &presence_enabled_config());
    let presence = evaluations
        .iter()
        .find(|evaluation| evaluation.check_id() == "constant-nonunit-scale")
        .expect("registered presence evaluation");
    assert_eq!(
        presence.evaluation(),
        animsmith_core::EvaluationState::Complete
    );
    assert_eq!(presence.findings().len(), 1);
    assert!(presence.gaps().is_empty());
    assert!(presence.evaluated_scopes().is_empty());
}

#[test]
fn constant_nonunit_scale_honors_each_enabling_severity() {
    let mut doc = clean_doc();
    add_scale_track(
        &mut doc,
        Interpolation::Linear,
        vec![0.0],
        vec![Vec3::splat(1.2)],
    );

    for (setting, expected) in [
        (SeveritySetting::Note, Severity::Note),
        (SeveritySetting::Warn, Severity::Warning),
        (SeveritySetting::Error, Severity::Error),
    ] {
        let findings = lint_with_config(&doc, &presence_enabled_config_with(setting));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "constant-nonunit-scale");
        assert_eq!(findings[0].severity, expected);
    }
}

#[test]
fn constant_uniform_nonunit_scale_is_redundancy_plus_opt_in_presence() {
    let mut doc = clean_doc();
    add_scale_track(
        &mut doc,
        Interpolation::Linear,
        vec![0.0, 0.5, 1.0],
        vec![Vec3::splat(1.2); 3],
    );
    assert_eq!(scale_finding_ids(&doc), vec!["constant-track"]);
    assert_eq!(
        scale_finding_ids_with_config(&doc, &presence_enabled_config()),
        vec!["constant-nonunit-scale", "constant-track"]
    );
}

#[test]
fn single_key_nonunit_pin_is_not_redundant() {
    let mut doc = clean_doc();
    add_scale_track(
        &mut doc,
        Interpolation::Linear,
        vec![0.0],
        vec![Vec3::splat(1.2)],
    );
    assert!(scale_finding_ids(&doc).is_empty());
    assert_eq!(
        scale_finding_ids_with_config(&doc, &presence_enabled_config()),
        vec!["constant-nonunit-scale"]
    );
    let enabled_findings = lint_with_config(&doc, &presence_enabled_config());
    assert_eq!(enabled_findings.len(), 1);
    assert_eq!(enabled_findings[0].check_id, "constant-nonunit-scale");
    assert!(
        enabled_findings
            .iter()
            .all(|finding| finding.check_id != "constant-track")
    );
}

#[test]
fn single_key_yz_nonuniform_pin_has_presence_and_nonuniform_owners() {
    let mut doc = clean_doc();
    add_scale_track(
        &mut doc,
        Interpolation::Linear,
        vec![0.0],
        vec![Vec3::new(1.0, 1.0, 1.2)],
    );
    assert_eq!(scale_finding_ids(&doc), vec!["non-uniform-scale"]);
    assert_eq!(
        scale_finding_ids_with_config(&doc, &presence_enabled_config()),
        vec!["constant-nonunit-scale", "non-uniform-scale"]
    );
}

#[test]
fn exact_and_over_tolerance_temporal_ranges_are_distinguished() {
    let immediately_over = f32::from_bits(1e-4f32.to_bits() + 1);
    for sign in [1.0, -1.0] {
        let mut exact = clean_doc();
        add_scale_track(
            &mut exact,
            Interpolation::Linear,
            vec![0.0, 1.0],
            vec![Vec3::ZERO, Vec3::splat(sign * 1e-4)],
        );
        assert!(
            !scale_finding_ids(&exact).contains(&"scale-keys"),
            "exact tolerance must remain constant"
        );

        let mut over = clean_doc();
        add_scale_track(
            &mut over,
            Interpolation::Linear,
            vec![0.0, 1.0],
            vec![Vec3::ZERO, Vec3::splat(sign * immediately_over)],
        );
        assert!(
            scale_finding_ids(&over).contains(&"scale-keys"),
            "the next stored value over tolerance must vary"
        );
    }
}

#[test]
fn exact_and_over_tolerance_non_uniform_spreads_are_distinguished() {
    let immediately_over = f32::from_bits(1e-4f32.to_bits() + 1);
    for sign in [1.0, -1.0] {
        let mut exact = clean_doc();
        add_scale_track(
            &mut exact,
            Interpolation::Linear,
            vec![0.0],
            vec![Vec3::new(sign * 1e-4, 0.0, 0.0)],
        );
        assert!(
            !scale_finding_ids(&exact).contains(&"non-uniform-scale"),
            "exact tolerance must remain uniform"
        );

        let mut over = clean_doc();
        add_scale_track(
            &mut over,
            Interpolation::Linear,
            vec![0.0],
            vec![Vec3::new(sign * immediately_over, 0.0, 0.0)],
        );
        assert!(
            scale_finding_ids(&over).contains(&"non-uniform-scale"),
            "the next component spread over tolerance must be non-uniform"
        );
    }
}

#[test]
fn stored_values_above_and_below_unit_tolerance_are_distinguished() {
    let unit = 1.0f32;
    let tolerance_pairs = [true, false].map(|increasing| {
        let mut tolerated = unit;
        loop {
            let next_bits = if increasing {
                tolerated.to_bits() + 1
            } else {
                tolerated.to_bits() - 1
            };
            let next = f32::from_bits(next_bits);
            if (next - unit).abs() > 1e-4 {
                break (tolerated, next);
            }
            tolerated = next;
        }
    });

    let enabled = presence_enabled_config();
    for (tolerated_value, over_value) in tolerance_pairs {
        assert!((tolerated_value - unit).abs() <= 1e-4);
        assert!((over_value - unit).abs() > 1e-4);

        let mut tolerated = clean_doc();
        add_scale_track(
            &mut tolerated,
            Interpolation::Linear,
            vec![0.0],
            vec![Vec3::splat(tolerated_value)],
        );
        assert!(
            !scale_finding_ids_with_config(&tolerated, &enabled)
                .contains(&"constant-nonunit-scale")
        );

        let mut over = clean_doc();
        add_scale_track(
            &mut over,
            Interpolation::Linear,
            vec![0.0],
            vec![Vec3::splat(over_value)],
        );
        let findings = lint_with_config(&over, &enabled);
        assert!(findings.iter().any(|finding| {
            finding.check_id == "constant-nonunit-scale" && finding.severity == Severity::Note
        }));
    }
}

#[test]
fn invalid_scale_tracks_are_left_to_source_data_checks() {
    let invalid_tracks = [
        (
            Track {
                bone: 1,
                property: Property::Scale,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::splat(f32::NAN)]),
            },
            Some("nan"),
        ),
        (
            Track {
                bone: 1,
                property: Property::Scale,
                interpolation: Interpolation::Step,
                times: vec![0.0, 0.0],
                values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::splat(1.2)]),
            },
            Some("time-monotonic"),
        ),
        (
            Track {
                bone: 1,
                property: Property::Scale,
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ONE, Vec3::ZERO]),
            },
            None,
        ),
        (
            Track {
                bone: 1,
                property: Property::Scale,
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::ZERO,
                    Vec3::ONE,
                    Vec3::splat(f32::NAN),
                    Vec3::ZERO,
                    Vec3::ONE,
                    Vec3::ZERO,
                ]),
            },
            Some("nan"),
        ),
    ];

    for (track, owning_check) in invalid_tracks {
        let mut doc = clean_doc();
        doc.clips[0].tracks.push(track);
        assert!(
            scale_finding_ids_with_config(&doc, &presence_enabled_config()).is_empty(),
            "invalid source data must not also be classified as a scale fact"
        );
        if let Some(owning_check) = owning_check {
            assert!(
                lint(&doc)
                    .iter()
                    .any(|finding| finding.check_id == owning_check),
                "{owning_check} must retain invalid-source ownership"
            );
        }
    }
}

#[test]
fn cubic_translation_tangents_do_not_count_as_redundant() {
    let mut doc = clean_doc();
    doc.clips[0].tracks[1] = Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::new(0.004, 0.0, 0.0),
            Vec3::new(0.004, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
        ]),
    };
    assert!(
        !lint(&doc)
            .iter()
            .any(|finding| finding.check_id == "constant-track"),
        "moving cubic translation must not be called redundant"
    );
}

#[test]
fn scale_findings_are_content_not_coverage_gaps_and_not_duplicated() {
    let mut doc = clean_doc();
    add_scale_track(
        &mut doc,
        Interpolation::CubicSpline,
        vec![0.0, 1.0],
        vec![
            Vec3::ZERO,
            Vec3::ONE,
            Vec3::new(0.004, 0.0, 0.0),
            Vec3::new(-0.004, 0.0, 0.0),
            Vec3::ONE,
            Vec3::ZERO,
        ],
    );
    let evaluations = evaluations_with_config(&doc, &Config::default());
    for check_id in ["scale-keys", "non-uniform-scale"] {
        let evaluation = evaluations
            .iter()
            .find(|evaluation| evaluation.check_id() == check_id)
            .expect("registered scale evaluation");
        assert_eq!(
            evaluation.evaluation(),
            animsmith_core::EvaluationState::Complete
        );
        assert_eq!(evaluation.findings().len(), 1);
        assert!(evaluation.gaps().is_empty());
        assert!(evaluation.evaluated_scopes().is_empty());
    }
}

#[test]
fn cubic_quaternion_tangents_do_not_count_as_redundant() {
    let mut doc = clean_doc();
    let q = Quat::IDENTITY;
    doc.clips[0].tracks[0] = Track {
        bone: 1,
        property: Property::Rotation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 1.0],
        values: TrackValues::Quats(vec![
            Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
            q,
            Quat::from_xyzw(0.1, 0.0, 0.0, 0.0),
            Quat::from_xyzw(0.1, 0.0, 0.0, 0.0),
            q,
            Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
        ]),
    };
    assert!(
        !lint(&doc)
            .iter()
            .any(|finding| finding.check_id == "constant-track"),
        "moving cubic quaternion must not be called redundant"
    );
}

#[test]
fn constant_rotation_track_is_noted() {
    let mut doc = clean_doc();
    let q = Quat::from_rotation_y(0.8);
    if let TrackValues::Quats(v) = &mut doc.clips[0].tracks[0].values {
        *v = vec![q, q, q];
    }
    assert_single(&doc, "constant-track", Severity::Note);
}

#[test]
fn measurements_report_rotation_range() {
    let doc = clean_doc();
    let config = Config::default();
    let grids = MetricGrids::new(&doc);
    let measurements =
        animsmith_core::measure::measure_document(&grids, &ResolvedRoles::default(), &config);
    let walk = &measurements["walk"];
    assert_eq!(walk.frame_count, 3);
    assert_eq!(walk.animated_bones, vec!["hips", "spine"]);
    let range = walk.bone_rotation_range_deg["spine"];
    let expected = 1.6f64.to_degrees();
    assert!(
        (range - expected).abs() < 0.1,
        "expected ~{expected}, got {range}"
    );
}
