//! Compare measurement maps and report per-metric movement beyond
//! significance thresholds.
//!
//! Per-metric significance thresholds treat movement below these values
//! as noise (f32 quantization, re-export dust), not a change worth
//! reporting.

use crate::measure::ClipMeasurements;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Duration movement threshold, in seconds.
pub const DURATION_THRESHOLD_S: f64 = 0.017; // half a frame at 30 fps
/// Bone rotation range movement threshold, in degrees.
pub const ROTATION_RANGE_THRESHOLD_DEG: f64 = 1.0;
/// Loop-seam ratio movement threshold.
pub const SEAM_THRESHOLD: f64 = 0.05;
/// Gait phase movement threshold, in circular cycle fraction.
pub const PHASE_THRESHOLD: f64 = 0.05; // cycle fraction, circular
/// Gait amplitude movement threshold, in metres.
pub const AMPLITUDE_THRESHOLD_M: f64 = 0.005;
/// Root-motion speed movement threshold, in metres per second.
pub const SPEED_THRESHOLD_MPS: f64 = 0.1;

/// One significant metric difference between two measurement maps.
#[derive(Debug, Serialize)]
pub struct MetricDelta {
    /// Clip that owns the changed metric, or the added/removed clip.
    pub clip: String,
    /// Metric path, for example `"duration_s"` or
    /// `"bone_rotation_range_deg[hips]"`.
    pub metric: String,
    /// Value in the before map, absent when a metric appeared or a clip
    /// was added/removed.
    #[serde(skip_serializing_if = "non_finite_or_none")]
    pub before: Option<f64>,
    /// Value in the after map, absent when a metric disappeared or a
    /// clip was added/removed.
    #[serde(skip_serializing_if = "non_finite_or_none")]
    pub after: Option<f64>,
    /// Short cause such as `"moved"`, `"appeared"`, or
    /// `"bone no longer animated"`.
    pub note: String,
}

fn non_finite_or_none(value: &Option<f64>) -> bool {
    value.is_none_or(|number| !number.is_finite())
}

/// Compare two measurement maps and return only significant deltas.
///
/// The thresholds are intentionally fixed public constants so CLI and
/// embedding callers agree on what counts as re-export noise. Gait phase
/// uses circular distance, so phases near `0.0` and `1.0` compare as
/// adjacent rather than far apart.
pub fn diff_measurements(
    a: &BTreeMap<String, ClipMeasurements>,
    b: &BTreeMap<String, ClipMeasurements>,
) -> Vec<MetricDelta> {
    let finite = |value: Option<f64>| value.filter(|value| value.is_finite());
    let mut deltas = Vec::new();
    let delta =
        |clip: &str, metric: &str, before: Option<f64>, after: Option<f64>, note: String| {
            MetricDelta {
                clip: clip.into(),
                metric: metric.into(),
                before,
                after,
                note,
            }
        };

    for (clip, ma) in a {
        let Some(mb) = b.get(clip) else {
            deltas.push(delta(clip, "clip", None, None, "clip removed".into()));
            continue;
        };
        let mut push_num =
            |metric: &str, va: Option<f64>, vb: Option<f64>, threshold: f64, circular: bool| {
                let moved = match (va, vb) {
                    (Some(x), Some(y)) => {
                        let d = if circular {
                            let d = (x - y).rem_euclid(1.0);
                            d.min(1.0 - d)
                        } else {
                            (x - y).abs()
                        };
                        d > threshold
                    }
                    (None, None) => false,
                    _ => true, // appeared or disappeared
                };
                if moved {
                    deltas.push(MetricDelta {
                        clip: clip.clone(),
                        metric: metric.into(),
                        before: va,
                        after: vb,
                        note: match (va, vb) {
                            (Some(_), Some(_)) => "moved".into(),
                            (None, Some(_)) => "appeared".into(),
                            _ => "disappeared".into(),
                        },
                    });
                }
            };

        push_num(
            "duration_s",
            finite(Some(ma.duration_s)),
            finite(Some(mb.duration_s)),
            DURATION_THRESHOLD_S,
            false,
        );
        push_num(
            "frame_count",
            Some(ma.frame_count as f64),
            Some(mb.frame_count as f64),
            0.5,
            false,
        );
        push_num(
            "loop_seam_ratio",
            finite(ma.loop_seam_ratio),
            finite(mb.loop_seam_ratio),
            SEAM_THRESHOLD,
            false,
        );
        push_num(
            "gait.phase",
            finite(ma.gait.as_ref().and_then(|g| g.phase)),
            finite(mb.gait.as_ref().and_then(|g| g.phase)),
            PHASE_THRESHOLD,
            true,
        );
        push_num(
            "gait.lr_amplitude_m",
            finite(ma.gait.as_ref().map(|g| g.lr_amplitude_m)),
            finite(mb.gait.as_ref().map(|g| g.lr_amplitude_m)),
            AMPLITUDE_THRESHOLD_M,
            false,
        );
        push_num(
            "speed_mps",
            finite(ma.speed_mps),
            finite(mb.speed_mps),
            SPEED_THRESHOLD_MPS,
            false,
        );

        for bone in ma
            .bone_rotation_range_deg
            .keys()
            .chain(mb.bone_rotation_range_deg.keys())
            .collect::<BTreeSet<_>>()
        {
            let va = finite(ma.bone_rotation_range_deg.get(bone).copied());
            let vb = finite(mb.bone_rotation_range_deg.get(bone).copied());
            let moved = match (va, vb) {
                (Some(x), Some(y)) => (x - y).abs() > ROTATION_RANGE_THRESHOLD_DEG,
                _ => true,
            };
            if moved {
                deltas.push(delta(
                    clip,
                    &format!("bone_rotation_range_deg[{bone}]"),
                    va,
                    vb,
                    match (va, vb) {
                        (Some(_), Some(_)) => "moved".into(),
                        (None, Some(_)) => "bone now animated".into(),
                        _ => "bone no longer animated".into(),
                    },
                ));
            }
        }

        if ma.animated_bones != mb.animated_bones {
            let a_set: BTreeSet<_> = ma.animated_bones.iter().collect();
            let b_set: BTreeSet<_> = mb.animated_bones.iter().collect();
            let gained: Vec<_> = b_set.difference(&a_set).map(|s| s.as_str()).collect();
            let lost: Vec<_> = a_set.difference(&b_set).map(|s| s.as_str()).collect();
            deltas.push(delta(
                clip,
                "animated_bones",
                Some(ma.animated_bones.len() as f64),
                Some(mb.animated_bones.len() as f64),
                format!("gained [{}], lost [{}]", gained.join(", "), lost.join(", ")),
            ));
        }
    }
    for clip in b.keys() {
        if !a.contains_key(clip) {
            deltas.push(MetricDelta {
                clip: clip.clone(),
                metric: "clip".into(),
                before: None,
                after: None,
                note: "clip added".into(),
            });
        }
    }
    deltas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::{ClipMeasurements, GaitMeasurement};

    fn clip_measurements() -> ClipMeasurements {
        ClipMeasurements {
            duration_s: 1.0,
            frame_count: 31,
            animated_bones: vec!["hips".into()],
            bone_rotation_range_deg: BTreeMap::from([("hips".into(), 10.0)]),
            loop_seam_ratio: Some(0.2),
            gait: Some(GaitMeasurement {
                phase: Some(0.25),
                lr_amplitude_m: 0.1,
            }),
            speed_mps: Some(1.0),
        }
    }

    fn measurement_map(
        clip: &str,
        measurements: ClipMeasurements,
    ) -> BTreeMap<String, ClipMeasurements> {
        BTreeMap::from([(clip.into(), measurements)])
    }

    fn delta_for<'a>(deltas: &'a [MetricDelta], metric: &str) -> &'a MetricDelta {
        deltas
            .iter()
            .find(|d| d.metric == metric)
            .unwrap_or_else(|| {
                panic!(
                    "missing metric delta {metric}; got {:?}",
                    delta_metrics(deltas)
                )
            })
    }

    fn delta_metrics(deltas: &[MetricDelta]) -> Vec<&str> {
        deltas.iter().map(|d| d.metric.as_str()).collect()
    }

    #[test]
    fn reports_moved_appeared_and_disappeared_metrics() {
        let mut before = clip_measurements();
        before.speed_mps = None;

        let mut after = before.clone();
        after.duration_s += DURATION_THRESHOLD_S * 2.0;
        after.loop_seam_ratio = None;
        after.speed_mps = Some(1.0);

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 3, "{:?}", delta_metrics(&deltas));
        assert_eq!(delta_for(&deltas, "duration_s").note, "moved");
        assert_eq!(delta_for(&deltas, "loop_seam_ratio").note, "disappeared");
        assert_eq!(delta_for(&deltas, "speed_mps").note, "appeared");
    }

    #[test]
    fn reports_clip_added_and_removed() {
        let deltas = diff_measurements(
            &measurement_map("removed", clip_measurements()),
            &measurement_map("added", clip_measurements()),
        );

        assert_eq!(deltas.len(), 2, "{:?}", delta_metrics(&deltas));
        assert!(
            deltas
                .iter()
                .any(|d| d.clip == "removed" && d.metric == "clip" && d.note == "clip removed")
        );
        assert!(
            deltas
                .iter()
                .any(|d| d.clip == "added" && d.metric == "clip" && d.note == "clip added")
        );
    }

    /// #52: anchor every documented threshold to literal stimuli.
    /// Deriving a metric's fixture from the constant under test
    /// (`THRESHOLD * 2`, `THRESHOLD / 2`) hides a fat-fingered constant:
    /// for example, `DURATION_THRESHOLD_S` 0.017 -> 0.17 would still pass.
    /// Concrete numbers straddling the documented threshold catch such a
    /// typo in either direction. `gait.phase` (circular) and `frame_count`
    /// (integer) do not fit this over/under numeric straddle; each has its
    /// own literal anchor.
    #[test]
    fn literal_stimuli_pin_documented_thresholds() {
        // Base fixture: duration_s 1.0, loop_seam_ratio 0.2,
        // lr_amplitude_m 0.1, speed_mps 1.0, hips rotation 10.0.
        struct Case {
            metric: &'static str,
            over: fn(&mut ClipMeasurements),  // clears the threshold
            under: fn(&mut ClipMeasurements), // stays within noise
        }
        let cases = [
            Case {
                metric: "duration_s", // threshold 0.017 s
                over: |m| m.duration_s = 1.02,
                under: |m| m.duration_s = 1.01,
            },
            Case {
                metric: "loop_seam_ratio", // threshold 0.05
                over: |m| m.loop_seam_ratio = Some(0.27),
                under: |m| m.loop_seam_ratio = Some(0.23),
            },
            Case {
                metric: "gait.lr_amplitude_m", // threshold 0.005 m
                over: |m| m.gait.as_mut().unwrap().lr_amplitude_m = 0.11,
                under: |m| m.gait.as_mut().unwrap().lr_amplitude_m = 0.102,
            },
            Case {
                metric: "speed_mps", // threshold 0.1 m/s
                over: |m| m.speed_mps = Some(1.15),
                under: |m| m.speed_mps = Some(1.05),
            },
            Case {
                metric: "bone_rotation_range_deg[hips]", // threshold 1.0 deg
                over: |m| {
                    m.bone_rotation_range_deg.insert("hips".into(), 13.0);
                },
                under: |m| {
                    m.bone_rotation_range_deg.insert("hips".into(), 10.5);
                },
            },
        ];

        for case in cases {
            let before = clip_measurements();

            let mut over = before.clone();
            (case.over)(&mut over);
            let deltas = diff_measurements(
                &measurement_map("walk", before.clone()),
                &measurement_map("walk", over),
            );
            assert_eq!(
                delta_metrics(&deltas),
                vec![case.metric],
                "over-threshold literal must report exactly {}",
                case.metric
            );

            let mut under = before.clone();
            (case.under)(&mut under);
            let deltas = diff_measurements(
                &measurement_map("walk", before),
                &measurement_map("walk", under),
            );
            assert!(
                deltas.is_empty(),
                "under-threshold literal for {} must be silent: {:?}",
                case.metric,
                delta_metrics(&deltas)
            );
        }
    }

    #[test]
    fn compares_gait_phase_on_a_cycle() {
        let mut before = clip_measurements();
        before.gait.as_mut().unwrap().phase = Some(0.98);
        let mut after = before.clone();
        after.gait.as_mut().unwrap().phase = Some(0.02);

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert!(deltas.is_empty(), "{:?}", delta_metrics(&deltas));
    }

    #[test]
    fn reports_significant_gait_phase_moves() {
        let mut before = clip_measurements();
        before.gait.as_mut().unwrap().phase = Some(0.9);
        let mut after = before.clone();
        after.gait.as_mut().unwrap().phase = Some(0.1);

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "gait.phase");
        assert_eq!(delta.note, "moved");
        assert_eq!(delta.before, Some(0.9));
        assert_eq!(delta.after, Some(0.1));
    }

    /// #53: `frame_count` is the wrong-sign guard; a decrease must still
    /// report, so an impl that only diffed increases is caught.
    #[test]
    fn reports_frame_count_move_including_a_decrease() {
        let before = clip_measurements(); // frame_count 31
        let mut after = before.clone();
        after.frame_count = 20;

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "frame_count");
        assert_eq!(delta.note, "moved");
        assert_eq!(delta.before, Some(31.0));
        assert_eq!(delta.after, Some(20.0));
        assert!(
            delta.before.unwrap() > delta.after.unwrap(),
            "a decrease must be captured, not dropped"
        );
    }

    /// #52 item 2: pin the `frame_count` 0.5 threshold to a literal
    /// one-frame move. `frame_count` is integer-valued, so the tightest
    /// possible stimulus - a single-frame change - must report.
    #[test]
    fn single_frame_change_crosses_the_frame_count_threshold() {
        let before = clip_measurements(); // frame_count 31
        let mut after = before.clone();
        after.frame_count = 32; // +1 frame, the smallest possible move

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "frame_count");
        assert_eq!(delta.note, "moved");
        assert_eq!(delta.before, Some(31.0));
        assert_eq!(delta.after, Some(32.0));
    }

    #[test]
    fn reports_gait_amplitude_move() {
        let before = clip_measurements(); // lr_amplitude_m 0.1
        let mut after = before.clone();
        after.gait.as_mut().unwrap().lr_amplitude_m = 0.1 + AMPLITUDE_THRESHOLD_M * 2.0;

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "gait.lr_amplitude_m");
        assert_eq!(delta.note, "moved");
        assert_eq!(delta.before, Some(0.1));
        assert_eq!(delta.after, Some(0.1 + AMPLITUDE_THRESHOLD_M * 2.0));
    }

    #[test]
    fn reports_bone_rotation_range_moved() {
        let before = clip_measurements(); // hips: 10.0
        let mut after = before.clone();
        after
            .bone_rotation_range_deg
            .insert("hips".into(), 10.0 + ROTATION_RANGE_THRESHOLD_DEG * 2.0);

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "bone_rotation_range_deg[hips]");
        assert_eq!(delta.note, "moved");
        assert_eq!(delta.before, Some(10.0));
        assert_eq!(delta.after, Some(10.0 + ROTATION_RANGE_THRESHOLD_DEG * 2.0));
    }

    #[test]
    fn reports_bone_rotation_range_appeared_and_disappeared() {
        let before = clip_measurements();
        let mut after = before.clone();
        after.bone_rotation_range_deg.insert("spine".into(), 5.0);
        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );
        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "bone_rotation_range_deg[spine]");
        assert_eq!(delta.note, "bone now animated");
        assert_eq!(delta.before, None);
        assert_eq!(delta.after, Some(5.0));

        let before = clip_measurements();
        let mut after = before.clone();
        after.bone_rotation_range_deg.remove("hips");
        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );
        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "bone_rotation_range_deg[hips]");
        assert_eq!(delta.note, "bone no longer animated");
        assert_eq!(delta.before, Some(10.0));
        assert_eq!(delta.after, None);
    }

    #[test]
    fn reports_animated_bones_gained_and_lost() {
        let before = clip_measurements(); // ["hips"]
        let mut after = before.clone();
        after.animated_bones = vec!["spine".into(), "tail".into()];

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "animated_bones");
        assert_eq!(delta.before, Some(1.0));
        assert_eq!(delta.after, Some(2.0));
        assert_eq!(delta.note, "gained [spine, tail], lost [hips]");
    }

    #[test]
    fn metric_delta_omits_non_finite_public_values() {
        let delta = MetricDelta {
            clip: "walk".into(),
            metric: "duration_s".into(),
            before: Some(f64::NAN),
            after: Some(f64::INFINITY),
            note: "moved".into(),
        };
        let json = serde_json::to_value(delta).expect("delta serializes");
        assert!(json.get("before").is_none());
        assert!(json.get("after").is_none());
    }
}
