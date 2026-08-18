//! Compare measurement maps and report per-metric movement beyond
//! significance thresholds.
//!
//! Per-metric significance thresholds treat movement below these values
//! as noise (f32 quantization, re-export dust), not a change worth
//! reporting.

use crate::measure::{ClipMeasurements, MeasurementAvailability};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Duration movement threshold, in seconds.
pub const DURATION_THRESHOLD_S: f64 = 0.017; // half a frame at 30 fps
/// Bone rotation range movement threshold, in degrees.
pub const ROTATION_RANGE_THRESHOLD_DEG: f64 = 1.0;
/// Per-bone loop-closure position movement threshold, in metres.
pub const LOOP_POSITION_THRESHOLD_M: f64 = 0.001;
/// Per-bone loop-closure rotation movement threshold, in degrees.
pub const LOOP_ROTATION_THRESHOLD_DEG: f64 = 0.1;
/// Per-bone seam-velocity movement threshold, in metres per second.
pub const LOOP_VELOCITY_THRESHOLD_MPS: f64 = 0.01;
/// Per-bone seam angular-velocity movement threshold, in degrees per second.
pub const LOOP_ANGULAR_VELOCITY_THRESHOLD_DEGPS: f64 = 0.5;
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
    /// Value in the before map, absent when a metric appeared, a clip was
    /// added/removed, or a publicly constructed delta carries a non-finite
    /// value that cannot be represented by the JSON contract.
    #[serde(skip_serializing_if = "non_finite_or_none")]
    pub before: Option<f64>,
    /// Value in the after map, absent when a metric disappeared, a clip was
    /// added/removed, or a publicly constructed delta carries a non-finite
    /// value that cannot be represented by the JSON contract.
    #[serde(skip_serializing_if = "non_finite_or_none")]
    pub after: Option<f64>,
    /// Short cause such as `"moved"`, `"appeared"`, or
    /// `"bone no longer animated"`.
    pub note: String,
}

fn non_finite_or_none(value: &Option<f64>) -> bool {
    value.is_none_or(|number| !number.is_finite())
}

/// Report any change in a clip fact's availability status
/// (`measured`/`not_applicable`/`unavailable`), including the
/// `not_applicable` <-> `unavailable` transition that a bare optional value
/// compares as unchanged elsewhere in this module (both are an absent
/// value), and a `measured` <-> absent transition for facts with no numeric
/// or structured value comparison of their own (`loop_endpoint_mode`,
/// `frame_grid`) — without this those two facts' appearance/disappearance
/// would be completely silent. The note names only the status reached: it is
/// deterministic given `(a, b)` because this function returns early when
/// `a == b`, so a reached status always names a genuine change.
///
/// `metric` should carry the wire field's own `_availability` suffix (for
/// example `"loop_seam_ratio_availability"`) so this status delta never
/// collides with a sibling numeric/structured delta reported under the bare
/// field name.
fn push_availability(
    deltas: &mut Vec<MetricDelta>,
    clip: &str,
    metric: &str,
    a: MeasurementAvailability,
    b: MeasurementAvailability,
) {
    if a == b {
        return;
    }
    let note = match b {
        MeasurementAvailability::Measured => "became measured",
        MeasurementAvailability::NotApplicable => "no longer applicable",
        MeasurementAvailability::Unavailable => "became unavailable",
    };
    deltas.push(MetricDelta {
        clip: clip.into(),
        metric: metric.into(),
        before: None,
        after: None,
        note: note.into(),
    });
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

        let a_loop_bones: BTreeMap<_, _> = ma
            .loop_continuity
            .as_ref()
            .into_iter()
            .flat_map(|continuity| &continuity.bones)
            .map(|bone| (bone.bone_index, bone))
            .collect();
        let b_loop_bones: BTreeMap<_, _> = mb
            .loop_continuity
            .as_ref()
            .into_iter()
            .flat_map(|continuity| &continuity.bones)
            .map(|bone| (bone.bone_index, bone))
            .collect();
        for bone_index in a_loop_bones
            .keys()
            .chain(b_loop_bones.keys())
            .copied()
            .collect::<BTreeSet<_>>()
        {
            let a_bone = a_loop_bones.get(&bone_index).copied();
            let b_bone = b_loop_bones.get(&bone_index).copied();
            let metric = |field: &str| format!("loop_continuity.bones[{bone_index}].{field}");
            push_num(
                &metric("position_delta_m"),
                finite(a_bone.map(|bone| bone.position_delta_m)),
                finite(b_bone.map(|bone| bone.position_delta_m)),
                LOOP_POSITION_THRESHOLD_M,
                false,
            );
            push_num(
                &metric("rotation_delta_deg"),
                finite(a_bone.map(|bone| bone.rotation_delta_deg)),
                finite(b_bone.map(|bone| bone.rotation_delta_deg)),
                LOOP_ROTATION_THRESHOLD_DEG,
                false,
            );
            push_num(
                &metric("seam_velocity_delta_mps"),
                finite(a_bone.map(|bone| bone.seam_velocity_delta_mps)),
                finite(b_bone.map(|bone| bone.seam_velocity_delta_mps)),
                LOOP_VELOCITY_THRESHOLD_MPS,
                false,
            );
            push_num(
                &metric("seam_angular_velocity_delta_degps"),
                finite(a_bone.map(|bone| bone.seam_angular_velocity_delta_degps)),
                finite(b_bone.map(|bone| bone.seam_angular_velocity_delta_degps)),
                LOOP_ANGULAR_VELOCITY_THRESHOLD_DEGPS,
                false,
            );
        }

        push_availability(
            &mut deltas,
            clip,
            "loop_continuity_availability",
            ma.loop_continuity_availability,
            mb.loop_continuity_availability,
        );
        push_availability(
            &mut deltas,
            clip,
            "loop_endpoint_mode_availability",
            ma.loop_endpoint_mode_availability,
            mb.loop_endpoint_mode_availability,
        );
        push_availability(
            &mut deltas,
            clip,
            "frame_grid_availability",
            ma.frame_grid_availability,
            mb.frame_grid_availability,
        );
        push_availability(
            &mut deltas,
            clip,
            "loop_seam_ratio_availability",
            ma.loop_seam_ratio_availability,
            mb.loop_seam_ratio_availability,
        );
        push_availability(
            &mut deltas,
            clip,
            "gait_availability",
            ma.gait_availability,
            mb.gait_availability,
        );
        push_availability(
            &mut deltas,
            clip,
            "speed_mps_availability",
            ma.speed_mps_availability,
            mb.speed_mps_availability,
        );
        // `gait.phase_availability` only exists as a field when the parent
        // `gait` object is present (schema: `gait` is Some exactly when
        // `gait_availability == measured`), so it is only meaningfully
        // comparable when both sides carry a `gait` object. A `gait` object
        // appearing/disappearing entirely is already reported above via
        // `gait_availability`, and `gait.phase`'s own value transition is
        // already reported via the numeric `gait.phase` delta.
        if let (Some(ga), Some(gb)) = (ma.gait.as_ref(), mb.gait.as_ref()) {
            push_availability(
                &mut deltas,
                clip,
                "gait.phase_availability",
                ga.phase_availability,
                gb.phase_availability,
            );
        }

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
                (None, None) => false,
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
    use crate::measure::{
        BoneLoopContinuityMeasurement, ClipMeasurements, FrameGridMeasurement, GaitMeasurement,
        LoopContinuityMeasurement, LoopEndpointMode, MeasurementAvailability,
    };

    fn clip_measurements() -> ClipMeasurements {
        ClipMeasurements {
            duration_s: 1.0,
            frame_count: 31,
            animated_bones: vec!["hips".into()],
            bone_rotation_range_deg: BTreeMap::from([("hips".into(), 10.0)]),
            loop_continuity: Some(LoopContinuityMeasurement {
                bones: vec![BoneLoopContinuityMeasurement {
                    bone_index: 0,
                    bone_name: "hips".into(),
                    position_delta_m: 0.02,
                    rotation_delta_deg: 2.0,
                    seam_velocity_delta_mps: 0.2,
                    seam_angular_velocity_delta_degps: 10.0,
                }],
            }),
            loop_continuity_availability: MeasurementAvailability::Measured,
            loop_endpoint_mode: None,
            loop_endpoint_mode_availability: MeasurementAvailability::NotApplicable,
            frame_grid: None,
            frame_grid_availability: MeasurementAvailability::NotApplicable,
            loop_seam_ratio: Some(0.2),
            loop_seam_ratio_availability: MeasurementAvailability::Measured,
            gait: Some(GaitMeasurement {
                phase: Some(0.25),
                phase_availability: MeasurementAvailability::Measured,
                lr_amplitude_m: 0.1,
            }),
            gait_availability: MeasurementAvailability::Measured,
            speed_mps: Some(1.0),
            speed_mps_availability: MeasurementAvailability::Measured,
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
    fn reports_per_bone_loop_continuity_appearance_by_stable_index() {
        let before = clip_measurements();
        let mut after = before.clone();
        after.loop_continuity = None;

        let disappeared = diff_measurements(
            &measurement_map("walk", before.clone()),
            &measurement_map("walk", after.clone()),
        );
        assert_eq!(disappeared.len(), 4, "{:?}", delta_metrics(&disappeared));
        assert!(disappeared.iter().all(|delta| delta.note == "disappeared"));
        assert!(
            disappeared
                .iter()
                .all(|delta| { delta.metric.starts_with("loop_continuity.bones[0].") })
        );

        let appeared = diff_measurements(
            &measurement_map("walk", after),
            &measurement_map("walk", before),
        );
        assert_eq!(appeared.len(), 4, "{:?}", delta_metrics(&appeared));
        assert!(appeared.iter().all(|delta| delta.note == "appeared"));
    }

    #[test]
    fn compares_nonzero_loop_bone_indices_even_when_names_repeat() {
        let mut before = clip_measurements();
        before
            .loop_continuity
            .as_mut()
            .unwrap()
            .bones
            .push(BoneLoopContinuityMeasurement {
                bone_index: 1,
                bone_name: "hips".into(),
                position_delta_m: 0.03,
                rotation_delta_deg: 3.0,
                seam_velocity_delta_mps: 0.3,
                seam_angular_velocity_delta_degps: 11.0,
            });
        let mut after = before.clone();
        let changed = &mut after.loop_continuity.as_mut().unwrap().bones[1];
        changed.position_delta_m = 0.032;
        changed.rotation_delta_deg = 3.2;
        changed.seam_velocity_delta_mps = 0.32;
        changed.seam_angular_velocity_delta_degps = 11.6;

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );
        assert_eq!(deltas.len(), 4, "{:?}", delta_metrics(&deltas));
        assert!(
            deltas
                .iter()
                .all(|delta| delta.metric.starts_with("loop_continuity.bones[1]."))
        );
    }

    #[test]
    fn loop_continuity_diff_floors_are_inclusive() {
        let mut before = clip_measurements();
        let before_bone = &mut before.loop_continuity.as_mut().unwrap().bones[0];
        before_bone.position_delta_m = 0.0;
        before_bone.rotation_delta_deg = 0.0;
        before_bone.seam_velocity_delta_mps = 0.0;
        before_bone.seam_angular_velocity_delta_degps = 0.0;

        let mut at_floor = before.clone();
        let after_bone = &mut at_floor.loop_continuity.as_mut().unwrap().bones[0];
        after_bone.position_delta_m = 0.001;
        after_bone.rotation_delta_deg = 0.1;
        after_bone.seam_velocity_delta_mps = 0.01;
        after_bone.seam_angular_velocity_delta_degps = 0.5;

        assert!(
            diff_measurements(
                &measurement_map("walk", before),
                &measurement_map("walk", at_floor),
            )
            .is_empty(),
            "movement exactly at a significance floor is noise"
        );
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
        // lr_amplitude_m 0.1, speed_mps 1.0, hips rotation 10.0,
        // loop position 0.02 m, rotation 2.0 deg, velocity 0.2 m/s.
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
            Case {
                metric: "loop_continuity.bones[0].position_delta_m", // threshold 0.001 m
                over: |m| {
                    m.loop_continuity.as_mut().unwrap().bones[0].position_delta_m = 0.022;
                },
                under: |m| {
                    m.loop_continuity.as_mut().unwrap().bones[0].position_delta_m = 0.0205;
                },
            },
            Case {
                metric: "loop_continuity.bones[0].rotation_delta_deg", // threshold 0.1 deg
                over: |m| {
                    m.loop_continuity.as_mut().unwrap().bones[0].rotation_delta_deg = 2.2;
                },
                under: |m| {
                    m.loop_continuity.as_mut().unwrap().bones[0].rotation_delta_deg = 2.05;
                },
            },
            Case {
                metric: "loop_continuity.bones[0].seam_velocity_delta_mps", // threshold 0.01 m/s
                over: |m| {
                    m.loop_continuity.as_mut().unwrap().bones[0].seam_velocity_delta_mps = 0.22;
                },
                under: |m| {
                    m.loop_continuity.as_mut().unwrap().bones[0].seam_velocity_delta_mps = 0.205;
                },
            },
            Case {
                metric: "loop_continuity.bones[0].seam_angular_velocity_delta_degps", // threshold 0.5 deg/s
                over: |m| {
                    m.loop_continuity.as_mut().unwrap().bones[0]
                        .seam_angular_velocity_delta_degps = 10.5001;
                },
                under: |m| {
                    m.loop_continuity.as_mut().unwrap().bones[0]
                        .seam_angular_velocity_delta_degps = 10.4999;
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

    #[test]
    fn reports_not_applicable_to_unavailable_availability_transitions() {
        let mut before = clip_measurements();
        before.speed_mps = None;
        before.speed_mps_availability = MeasurementAvailability::NotApplicable;

        let mut after = before.clone();
        after.speed_mps_availability = MeasurementAvailability::Unavailable;

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "speed_mps_availability");
        assert_eq!(delta.note, "became unavailable");
        assert_eq!(delta.before, None);
        assert_eq!(delta.after, None);
    }

    #[test]
    fn reports_unavailable_to_not_applicable_availability_transitions() {
        let mut before = clip_measurements();
        before.speed_mps = None;
        before.speed_mps_availability = MeasurementAvailability::Unavailable;

        let mut after = before.clone();
        after.speed_mps_availability = MeasurementAvailability::NotApplicable;

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "speed_mps_availability");
        assert_eq!(delta.note, "no longer applicable");
    }

    #[test]
    fn availability_stays_silent_only_when_unchanged() {
        let before = clip_measurements();
        let after = before.clone();

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );
        assert!(deltas.is_empty(), "{:?}", delta_metrics(&deltas));
    }

    /// A `measured` -> `unavailable` transition on a scalar fact reports
    /// *two* deltas: the ordinary numeric "disappeared" delta under the
    /// bare field name, and a status delta under the field's
    /// `_availability` name. Both are contract evidence: the first is the
    /// value movement a consumer already watches; the second is what #443
    /// exists for — it is the only signal that distinguishes this
    /// applicable-but-failed-to-derive case from a legitimate absence.
    #[test]
    fn measured_to_unavailable_reports_both_value_and_availability_deltas() {
        let before = clip_measurements();
        let mut after = before.clone();
        after.speed_mps = None;
        after.speed_mps_availability = MeasurementAvailability::Unavailable;

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 2, "{:?}", delta_metrics(&deltas));
        assert_eq!(delta_for(&deltas, "speed_mps").note, "disappeared");
        assert_eq!(
            delta_for(&deltas, "speed_mps_availability").note,
            "became unavailable"
        );
    }

    #[test]
    fn reports_gait_phase_not_applicable_to_unavailable_transitions() {
        let mut before = clip_measurements();
        before.gait.as_mut().unwrap().phase = None;
        before.gait.as_mut().unwrap().phase_availability = MeasurementAvailability::NotApplicable;

        let mut after = before.clone();
        after.gait.as_mut().unwrap().phase_availability = MeasurementAvailability::Unavailable;

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 1, "{:?}", delta_metrics(&deltas));
        let delta = delta_for(&deltas, "gait.phase_availability");
        assert_eq!(delta.note, "became unavailable");
    }

    /// `gait.phase_availability` is a field nested inside the `gait` object,
    /// so it exists only when `gait` itself is present (schema invariant:
    /// `gait` is `Some` exactly when `gait_availability == measured`).
    /// When `gait` disappears entirely there is no `gait.phase_availability`
    /// to compare on the side that lost it, so this transition must not be
    /// reported there — the parent `gait_availability` delta and the
    /// numeric `gait.phase` "disappeared" delta already carry that event.
    #[test]
    fn gait_phase_availability_is_not_compared_when_gait_itself_disappears() {
        let before = clip_measurements();
        let mut after = before.clone();
        after.gait = None;
        after.gait_availability = MeasurementAvailability::Unavailable;

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert!(
            deltas
                .iter()
                .all(|delta| delta.metric != "gait.phase_availability"),
            "gait.phase_availability has no valid comparison when gait is absent on one side: {:?}",
            delta_metrics(&deltas)
        );
        assert_eq!(delta_for(&deltas, "gait.phase").note, "disappeared");
        assert_eq!(
            delta_for(&deltas, "gait_availability").note,
            "became unavailable"
        );
    }

    /// `loop_endpoint_mode` and `frame_grid` have no numeric or structured
    /// value comparison of their own (they are an enum and a small object,
    /// not an `f64`), so a `measured` -> `unavailable` transition on either
    /// would otherwise be completely silent. The `_availability` status
    /// delta is their only diff evidence.
    #[test]
    fn measured_to_unavailable_reports_endpoint_and_frame_grid_status_deltas() {
        let mut before = clip_measurements();
        before.loop_endpoint_mode = Some(LoopEndpointMode::UniqueCycle);
        before.loop_endpoint_mode_availability = MeasurementAvailability::Measured;
        before.frame_grid = Some(FrameGridMeasurement {
            fps: 30.0,
            frame_intervals: 30,
        });
        before.frame_grid_availability = MeasurementAvailability::Measured;

        let mut after = before.clone();
        after.loop_endpoint_mode = None;
        after.loop_endpoint_mode_availability = MeasurementAvailability::Unavailable;
        after.frame_grid = None;
        after.frame_grid_availability = MeasurementAvailability::Unavailable;

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 2, "{:?}", delta_metrics(&deltas));
        assert_eq!(
            delta_for(&deltas, "loop_endpoint_mode_availability").note,
            "became unavailable"
        );
        assert_eq!(
            delta_for(&deltas, "frame_grid_availability").note,
            "became unavailable"
        );
    }

    #[test]
    fn reports_endpoint_and_frame_grid_not_applicable_to_unavailable_transitions() {
        let mut before = clip_measurements();
        before.loop_endpoint_mode_availability = MeasurementAvailability::NotApplicable;
        before.frame_grid_availability = MeasurementAvailability::NotApplicable;

        let mut after = before.clone();
        after.loop_endpoint_mode_availability = MeasurementAvailability::Unavailable;
        after.frame_grid_availability = MeasurementAvailability::Unavailable;

        let deltas = diff_measurements(
            &measurement_map("walk", before),
            &measurement_map("walk", after),
        );

        assert_eq!(deltas.len(), 2, "{:?}", delta_metrics(&deltas));
        assert_eq!(
            delta_for(&deltas, "loop_endpoint_mode_availability").note,
            "became unavailable"
        );
        assert_eq!(
            delta_for(&deltas, "frame_grid_availability").note,
            "became unavailable"
        );
    }
}
