use animsmith_core::diff::{
    AMPLITUDE_THRESHOLD_M, DURATION_THRESHOLD_S, MetricDelta, PHASE_THRESHOLD,
    ROTATION_RANGE_THRESHOLD_DEG, SEAM_THRESHOLD, SPEED_THRESHOLD_MPS, diff_measurements,
};
use animsmith_core::measure::ClipMeasurements;
use serde_json::json;
use std::collections::BTreeMap;

type MeasurementMap = BTreeMap<String, ClipMeasurements>;
type PublicDiffFn = fn(&MeasurementMap, &MeasurementMap) -> Vec<MetricDelta>;

const PUBLIC_DIFF_MEASUREMENTS: PublicDiffFn = diff_measurements;

fn measurements(duration_s: f64) -> MeasurementMap {
    serde_json::from_value(json!({
        "walk": {
            "duration_s": duration_s,
            "frame_count": 31,
            "animated_bones": ["hips"],
            "bone_rotation_range_deg": { "hips": 10.0 },
            "loop_seam_ratio": 0.2,
            "gait": {
                "phase": 0.25,
                "lr_amplitude_m": 0.1
            },
            "speed_mps": 1.0
        }
    }))
    .expect("valid public measurement map")
}

#[test]
fn public_diff_api_accepts_deserialized_measurements() {
    let deltas = PUBLIC_DIFF_MEASUREMENTS(&measurements(1.0), &measurements(1.1));

    assert_eq!(deltas.len(), 1, "{deltas:?}");
    let delta = &deltas[0];
    assert_eq!(delta.clip, "walk");
    assert_eq!(delta.metric, "duration_s");
    assert_eq!(delta.before, Some(1.0));
    assert_eq!(delta.after, Some(1.1));
    assert_eq!(delta.note, "moved");
}

#[test]
fn threshold_constants_are_public_and_unchanged() {
    assert_eq!(DURATION_THRESHOLD_S, 0.017);
    assert_eq!(ROTATION_RANGE_THRESHOLD_DEG, 1.0);
    assert_eq!(SEAM_THRESHOLD, 0.05);
    assert_eq!(PHASE_THRESHOLD, 0.05);
    assert_eq!(AMPLITUDE_THRESHOLD_M, 0.005);
    assert_eq!(SPEED_THRESHOLD_MPS, 0.1);
}

#[test]
fn public_diff_treats_non_finite_measurements_as_absent() {
    struct Case {
        metric: &'static str,
        finite: f64,
        make_non_finite: fn(&mut ClipMeasurements),
        appeared_note: &'static str,
        disappeared_note: &'static str,
    }
    let cases = [
        Case {
            metric: "duration_s",
            finite: 1.0,
            make_non_finite: |clip| clip.duration_s = f64::NAN,
            appeared_note: "appeared",
            disappeared_note: "disappeared",
        },
        Case {
            metric: "loop_seam_ratio",
            finite: 0.2,
            make_non_finite: |clip| clip.loop_seam_ratio = Some(f64::INFINITY),
            appeared_note: "appeared",
            disappeared_note: "disappeared",
        },
        Case {
            metric: "gait.phase",
            finite: 0.25,
            make_non_finite: |clip| {
                clip.gait.as_mut().expect("fixture gait").phase = Some(f64::NEG_INFINITY);
            },
            appeared_note: "appeared",
            disappeared_note: "disappeared",
        },
        Case {
            metric: "gait.lr_amplitude_m",
            finite: 0.1,
            make_non_finite: |clip| {
                clip.gait.as_mut().expect("fixture gait").lr_amplitude_m = f64::NAN;
            },
            appeared_note: "appeared",
            disappeared_note: "disappeared",
        },
        Case {
            metric: "speed_mps",
            finite: 1.0,
            make_non_finite: |clip| clip.speed_mps = Some(f64::INFINITY),
            appeared_note: "appeared",
            disappeared_note: "disappeared",
        },
        Case {
            metric: "bone_rotation_range_deg[hips]",
            finite: 10.0,
            make_non_finite: |clip| {
                clip.bone_rotation_range_deg.insert("hips".into(), f64::NAN);
            },
            appeared_note: "bone now animated",
            disappeared_note: "bone no longer animated",
        },
    ];

    for case in cases {
        let finite = measurements(1.0);
        let mut non_finite = finite.clone();
        (case.make_non_finite)(non_finite.get_mut("walk").expect("fixture clip"));

        assert!(
            PUBLIC_DIFF_MEASUREMENTS(&non_finite, &non_finite).is_empty(),
            "identical non-finite {} must not produce a false delta",
            case.metric
        );

        let disappeared = PUBLIC_DIFF_MEASUREMENTS(&finite, &non_finite);
        assert_eq!(disappeared.len(), 1, "{}: {disappeared:?}", case.metric);
        assert_eq!(disappeared[0].metric, case.metric);
        assert_eq!(disappeared[0].before, Some(case.finite));
        assert_eq!(disappeared[0].after, None);
        assert_eq!(disappeared[0].note, case.disappeared_note);

        let appeared = PUBLIC_DIFF_MEASUREMENTS(&non_finite, &finite);
        assert_eq!(appeared.len(), 1, "{}: {appeared:?}", case.metric);
        assert_eq!(appeared[0].metric, case.metric);
        assert_eq!(appeared[0].before, None);
        assert_eq!(appeared[0].after, Some(case.finite));
        assert_eq!(appeared[0].note, case.appeared_note);
    }
}
