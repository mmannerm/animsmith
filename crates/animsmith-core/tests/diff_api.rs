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
    let mut invalid = measurements(1.0);
    let clip = invalid.get_mut("walk").expect("fixture clip");
    clip.duration_s = f64::NAN;
    clip.loop_seam_ratio = Some(f64::INFINITY);
    clip.gait.as_mut().expect("fixture gait").phase = Some(f64::NEG_INFINITY);
    clip.gait.as_mut().expect("fixture gait").lr_amplitude_m = f64::NAN;
    clip.speed_mps = Some(f64::INFINITY);
    clip.bone_rotation_range_deg.insert("hips".into(), f64::NAN);

    assert!(
        PUBLIC_DIFF_MEASUREMENTS(&invalid, &invalid).is_empty(),
        "identical absent/non-finite values must not produce false deltas"
    );

    let finite = measurements(1.0);
    let disappeared = PUBLIC_DIFF_MEASUREMENTS(&finite, &invalid);
    let bone = disappeared
        .iter()
        .find(|delta| delta.metric == "bone_rotation_range_deg[hips]")
        .expect("finite-to-non-finite bone transition is reported");
    assert_eq!(bone.before, Some(10.0));
    assert_eq!(bone.after, None);
    assert_eq!(bone.note, "bone no longer animated");

    let appeared = PUBLIC_DIFF_MEASUREMENTS(&invalid, &finite);
    let bone = appeared
        .iter()
        .find(|delta| delta.metric == "bone_rotation_range_deg[hips]")
        .expect("non-finite-to-finite bone transition is reported");
    assert_eq!(bone.before, None);
    assert_eq!(bone.after, Some(10.0));
    assert_eq!(bone.note, "bone now animated");
}
