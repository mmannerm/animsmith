//! `animsmith diff A B` — compare the measurement maps of two inputs
//! (asset files measured on the fly, or prior `measure` JSON reports)
//! and report per-metric movement beyond significance thresholds.
//! Primary use: "did this DCC re-export change anything that matters?"

use animsmith_core::measure::ClipMeasurements;
use serde::Serialize;
use std::collections::BTreeMap;

/// Per-metric significance thresholds: movement below these is noise
/// (f32 quantization, re-export dust), not a change worth reporting.
pub const DURATION_THRESHOLD_S: f64 = 0.017; // half a frame at 30 fps
pub const ROTATION_RANGE_THRESHOLD_DEG: f64 = 1.0;
pub const SEAM_THRESHOLD: f64 = 0.05;
pub const PHASE_THRESHOLD: f64 = 0.05; // cycle fraction, circular
pub const AMPLITUDE_THRESHOLD_M: f64 = 0.005;
pub const SPEED_THRESHOLD_MPS: f64 = 0.1;

#[derive(Debug, Serialize)]
pub struct MetricDelta {
    pub clip: String,
    pub metric: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<f64>,
    pub note: String,
}

pub fn diff_measurements(
    a: &BTreeMap<String, ClipMeasurements>,
    b: &BTreeMap<String, ClipMeasurements>,
) -> Vec<MetricDelta> {
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
            Some(ma.duration_s),
            Some(mb.duration_s),
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
            ma.loop_seam_ratio,
            mb.loop_seam_ratio,
            SEAM_THRESHOLD,
            false,
        );
        push_num(
            "gait.phase",
            ma.gait.as_ref().and_then(|g| g.phase),
            mb.gait.as_ref().and_then(|g| g.phase),
            PHASE_THRESHOLD,
            true,
        );
        push_num(
            "gait.lr_amplitude_m",
            ma.gait.as_ref().map(|g| g.lr_amplitude_m),
            mb.gait.as_ref().map(|g| g.lr_amplitude_m),
            AMPLITUDE_THRESHOLD_M,
            false,
        );
        push_num(
            "speed_mps",
            ma.speed_mps,
            mb.speed_mps,
            SPEED_THRESHOLD_MPS,
            false,
        );

        for bone in ma
            .bone_rotation_range_deg
            .keys()
            .chain(mb.bone_rotation_range_deg.keys())
            .collect::<std::collections::BTreeSet<_>>()
        {
            let va = ma.bone_rotation_range_deg.get(bone).copied();
            let vb = mb.bone_rotation_range_deg.get(bone).copied();
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
            let a_set: std::collections::BTreeSet<_> = ma.animated_bones.iter().collect();
            let b_set: std::collections::BTreeSet<_> = mb.animated_bones.iter().collect();
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
    use animsmith_core::measure::ClipMeasurements;
    use serde_json::json;

    fn clip_measurements() -> ClipMeasurements {
        serde_json::from_value(json!({
            "duration_s": 1.0,
            "frame_count": 31,
            "animated_bones": ["hips"],
            "bone_rotation_range_deg": { "hips": 10.0 },
            "loop_seam_ratio": 0.2,
            "gait": {
                "phase": 0.25,
                "lr_amplitude_m": 0.1
            },
            "speed_mps": 1.0
        }))
        .expect("valid clip measurement fixture")
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

    /// #52: anchor every documented threshold to LITERAL stimuli.
    /// Deriving a metric's fixture from the constant under test
    /// (`THRESHOLD * 2`, `THRESHOLD / 2`) hides a fat-fingered constant —
    /// e.g. `DURATION_THRESHOLD_S` 0.017 → 0.17 would still pass. Concrete
    /// numbers straddling the documented threshold catch such a typo in
    /// either direction. `gait.phase` (circular) and `frame_count`
    /// (integer) don't fit this over/under numeric straddle; each has
    /// its own literal anchor — see
    /// `single_frame_change_crosses_the_frame_count_threshold`,
    /// `reports_significant_gait_phase_moves`, and
    /// `compares_gait_phase_on_a_cycle`.
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

    /// #53: `frame_count` is the wrong-sign guard — a *decrease* must
    /// still report, so an impl that only diffed increases is caught.
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

    /// #52 item 2: pin the `frame_count` 0.5 threshold to a LITERAL
    /// one-frame move. `frame_count` is integer-valued, so the tightest
    /// possible stimulus — a single-frame change — must report; a
    /// fat-fingered threshold (0.5 → 1.5) would silence it and fail
    /// here. The decrease test above proves the sign is handled; this
    /// proves the boundary sits below one frame.
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
        // A bone gaining a rotation range: before None, after Some.
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

        // A bone losing its rotation range: before Some, after None.
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
        // Exact note: set difference (sorted), not just a count change.
        assert_eq!(delta.note, "gained [spine, tail], lost [hips]");
    }
}
