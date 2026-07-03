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
pub const PHASE_THRESHOLD: f64 = 0.02; // cycle fraction, circular
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
