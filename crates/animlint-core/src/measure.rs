//! Measurements: the raw per-clip metric map that `measure` emits and
//! `lint` judges. Kept separate from findings so pipelines (e.g. a
//! bake's measured sidecar) can pin their own contracts to the numbers.

use crate::model::{Document, Property};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Rotation ranges below this are not recorded (matches the incubating
/// pipeline's convention).
pub const MIN_RECORDED_ROTATION_DEG: f64 = 0.1;

#[derive(Debug, Clone, Serialize)]
pub struct ClipMeasurements {
    pub duration_s: f64,
    /// Keyframe count of the longest channel.
    pub frame_count: u32,
    /// Bones with at least one keyframed channel, sorted.
    pub animated_bones: Vec<String>,
    /// Max rotation deviation (degrees) of each bone from its first
    /// keyed rotation. Bones under [`MIN_RECORDED_ROTATION_DEG`] are
    /// omitted.
    pub bone_rotation_range_deg: BTreeMap<String, f64>,
}

/// Measure every clip in the document.
pub fn measure_document(doc: &Document) -> BTreeMap<String, ClipMeasurements> {
    doc.clips
        .iter()
        .map(|clip| {
            let mut animated: BTreeSet<String> = BTreeSet::new();
            let mut rotation_range: BTreeMap<String, f64> = BTreeMap::new();
            let mut frame_count = 0usize;

            for track in &clip.tracks {
                let Some(bone) = doc.skeleton.bones.get(track.bone) else {
                    continue;
                };
                if track.key_count() == 0 {
                    continue;
                }
                animated.insert(bone.name.clone());
                frame_count = frame_count.max(track.key_count());

                if track.property == Property::Rotation
                    && let Some(first) = track.key_quat(0)
                    && first.is_finite()
                    && first.length_squared() > 0.0
                {
                    let first = first.normalize();
                    let mut max_deg = 0.0f64;
                    for k in 1..track.key_count() {
                        if let Some(q) = track.key_quat(k)
                            && q.is_finite()
                            && q.length_squared() > 0.0
                        {
                            let deg = first.angle_between(q.normalize()).to_degrees() as f64;
                            max_deg = max_deg.max(deg);
                        }
                    }
                    if max_deg >= MIN_RECORDED_ROTATION_DEG {
                        let entry = rotation_range.entry(bone.name.clone()).or_insert(0.0);
                        *entry = entry.max(max_deg);
                    }
                }
            }

            (
                clip.name.clone(),
                ClipMeasurements {
                    duration_s: clip.duration_s,
                    frame_count: frame_count as u32,
                    animated_bones: animated.into_iter().collect(),
                    bone_rotation_range_deg: rotation_range,
                },
            )
        })
        .collect()
}
