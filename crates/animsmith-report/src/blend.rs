//! Bounded, optional presentation authority. Never changes diagnostic admission.
use animsmith_core::{
    Document,
    model::{Clip, Interpolation, Property, TrackValues, Transform},
    sample::PoseGrid,
};
use base64::Engine as _;
use std::collections::BTreeSet;

pub(crate) const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_BONES: usize = 1024;
const MAX_CLIPS: usize = 4096;
const MAX_TRACKS: usize = 65536;
const MAX_BASE64: usize = 44_750_164;
pub(crate) const BUDGET_NOTICE: &str = "Illustrative blending omitted: added local-transform data exceeds the report blend budget. Source playback and findings remain available. Select fewer clips to include blending.";

// Fed by the existing report traversal. Bounds are checked before inspecting
// track metadata, sampled locals or allocating streams. Excluded clips do not
// incur a second traversal, and no new sampling call is needed.
#[derive(Default)]
pub(crate) struct Preflight {
    clips: usize,
    tracks: usize,
    raw: usize,
    encoded: usize,
    reason: Option<&'static str>,
}
impl Preflight {
    pub(crate) fn include(&mut self, clip: &Clip) {
        if self.reason.is_some() {
            return;
        }
        self.clips += 1;
        self.tracks = self.tracks.saturating_add(clip.tracks.len());
        if self.clips > MAX_CLIPS || self.tracks > MAX_TRACKS {
            self.reason = Some(BUDGET_NOTICE);
        }
    }
    pub(crate) fn grid(&mut self, bones: usize, frames: usize) -> bool {
        if self.reason.is_some() {
            return false;
        }
        let counts = local_bytes(bones, frames).and_then(|bytes| {
            Some((
                self.raw.checked_add(bytes)?,
                self.encoded.checked_add(base64_bytes(bytes)?)?,
            ))
        });
        match counts {
            Some((raw, encoded)) if raw <= MAX_BYTES && encoded <= MAX_BASE64 => {
                self.raw = raw;
                self.encoded = encoded;
                true
            }
            _ => {
                self.reason = Some(BUDGET_NOTICE);
                false
            }
        }
    }
    pub(crate) fn finish(self, doc: &Document) -> Result<(), &'static str> {
        if let Some(reason) = self.reason {
            return Err(reason);
        }
        let bones = doc.skeleton.bones.len();
        if bones == 0 || bones > MAX_BONES {
            return Err(
                "Illustrative blending omitted: skeleton must contain 1 to 1024 bones. Source playback and findings remain available.",
            );
        }
        if doc
            .skeleton
            .bones
            .iter()
            .enumerate()
            .any(|(i, b)| b.parent.is_some_and(|p| p >= i))
        {
            return Err(
                "Illustrative blending omitted: invalid parent hierarchy. Source playback and findings remain available.",
            );
        }
        Ok(())
    }
}
#[cfg(test)]
fn preflight(doc: &Document, filter: Option<&str>) -> Result<(), &'static str> {
    let mut p = Preflight::default();
    for c in doc
        .clips
        .iter()
        .filter(|c| filter.is_none_or(|f| f == c.name))
    {
        p.include(c);
        if let Some(frames) = animsmith_core::metrics::metric_frame_count(c) {
            p.grid(doc.skeleton.bones.len(), frames);
        }
    }
    p.finish(doc)
}

fn local_bytes(bones: usize, frames: usize) -> Option<usize> {
    bones.checked_mul(frames)?.checked_mul(40)
}
fn base64_bytes(bytes: usize) -> Option<usize> {
    bytes.checked_add(2)?.checked_div(3)?.checked_mul(4)
}
fn fields(t: Transform) -> [f32; 10] {
    [
        t.translation.x,
        t.translation.y,
        t.translation.z,
        t.rotation.x,
        t.rotation.y,
        t.rotation.z,
        t.rotation.w,
        t.scale.x,
        t.scale.y,
        t.scale.z,
    ]
}

pub(crate) fn encode(clip: &Clip, grid: &PoseGrid) -> Result<String, &'static str> {
    if !clip.duration_s.is_finite() {
        return Err("non-finite duration");
    }
    let mut channels = BTreeSet::new();
    for track in &clip.tracks {
        let cardinality =
            track
                .times
                .len()
                .checked_mul(if track.interpolation == Interpolation::CubicSpline {
                    3
                } else {
                    1
                });
        let kind_ok = matches!(
            (&track.property, &track.values),
            (
                Property::Translation | Property::Scale,
                TrackValues::Vec3s(_)
            ) | (Property::Rotation, TrackValues::Quats(_))
        );
        if track.bone >= grid.bone_count()
            || !kind_ok
            || track.times.is_empty()
            || cardinality != Some(track.values.len())
            || !channels.insert((track.bone, track.property))
        {
            return Err(
                "malformed track metadata (target, property, storage, cardinality or duplicate channel)",
            );
        }
    }
    // Validate all records before binary/base64 allocation; missing channels have
    // already inherited their rest components from the existing Rust sampler.
    for f in 0..grid.frame_count() {
        if !grid.times[f].is_finite() {
            return Err("non-finite sample time");
        }
        for b in 0..grid.bone_count() {
            let t = grid.local(f, b);
            if !fields(t).into_iter().all(f32::is_finite) {
                return Err("non-finite sampled local transform");
            }
            let q = t.rotation.to_array().map(f64::from);
            let norm = q.into_iter().map(|x| x * x).sum::<f64>().sqrt();
            if !norm.is_finite() || norm == 0.0 || (norm - 1.0).abs() > 1e-4 {
                return Err("sampled quaternion length is outside 1 +/- 1e-4");
            }
        }
    }
    let bytes = local_bytes(grid.bone_count(), grid.frame_count()).ok_or(BUDGET_NOTICE)?;
    if bytes > MAX_BYTES {
        return Err(BUDGET_NOTICE);
    }
    let mut raw = Vec::with_capacity(bytes);
    for f in 0..grid.frame_count() {
        for b in 0..grid.bone_count() {
            for value in fields(grid.local(f, b)) {
                raw.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReportInputs, ReportOptions, render};
    use animsmith_core::{
        Config, MetricGrids, ResolvedRoles,
        glam::{Quat, Vec3},
        model::{Bone, Skeleton, Track},
    };
    use serde_json::Value;
    fn document() -> Document {
        Document {
            skeleton: Skeleton {
                bones: vec![Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform {
                        translation: Vec3::new(1.0, 2.0, 3.0),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(-1.0, 0.0, 2.0),
                    },
                    inverse_bind: None,
                }],
            },
            clips: vec![Clip {
                name: "a".into(),
                duration_s: 1.0,
                tracks: vec![Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![Vec3::new(4.0, 5.0, 6.0); 3]),
                }],
            }],
            ..Document::default()
        }
    }
    fn payload(doc: &Document, evidence: bool, filter: Option<&str>) -> Value {
        let grids = MetricGrids::new(doc);
        let roles = ResolvedRoles::default();
        let config = Config::default();
        let checks = [animsmith_core::CheckEvaluation::evaluated(
            "blend-fixture",
            animsmith_core::CheckOutput::from_coverage(
                vec![animsmith_core::Finding::new(
                    "blend-fixture",
                    animsmith_core::Severity::Warning,
                    "source finding survives blend omission",
                )],
                vec![],
                vec![],
            ),
        )
        .unwrap()];
        let html = render(ReportInputs {
            options: ReportOptions {
                evidence_only: evidence,
            },
            clip: filter,
            ..ReportInputs::new(&grids, &roles, &checks, &config)
        });
        let raw = html
            .split_once("<script type=\"application/json\" id=\"report-data\">")
            .unwrap()
            .1
            .split_once("</script>")
            .unwrap()
            .0;
        serde_json::from_str(raw).unwrap()
    }
    #[test]
    fn layout_missing_channels_and_evidence_only() {
        let doc = document();
        let full = payload(&doc, false, None);
        let evidence = payload(&doc, true, None);
        let raw = base64::engine::general_purpose::STANDARD
            .decode(full["clips"][0]["locals"].as_str().unwrap())
            .unwrap();
        assert_eq!(raw.len(), 3 * 40);
        let expected = [4.0f32, 5.0, 6.0, 0.0, 0.0, 0.0, 1.0, -1.0, 0.0, 2.0];
        for frame in raw.chunks_exact(40) {
            for (value, bytes) in expected.iter().zip(frame.chunks_exact(4)) {
                assert_eq!(value.to_le_bytes(), bytes);
            }
        }
        assert_eq!(full["blend"]["raw_bytes"], 120);
        assert_eq!(full["blend"]["base64_bytes"], 160);
        assert!(full.get("rest").is_none());
        assert!(evidence.get("blend").is_none());
        assert!(evidence["clips"][0].get("locals").is_none());
        assert!(evidence["clips"][0].get("positions").is_none());
        assert_eq!(full["findings"], evidence["findings"]);
    }
    #[test]
    fn invalid_clip_omission_preserves_other_clips_and_source_data() {
        for mutation in 0..7 {
            let mut doc = document();
            let good = payload(&doc, false, None);
            let mut bad = doc.clips[0].clone();
            bad.name = "bad".into();
            match mutation {
                0 => bad.tracks[0].bone = 1,
                1 => bad.tracks[0].property = Property::Rotation,
                2 => bad.tracks[0].values = TrackValues::Vec3s(vec![]),
                3 => bad.tracks.push(bad.tracks[0].clone()),
                4 => bad.tracks[0].interpolation = Interpolation::CubicSpline,
                5 => bad.tracks[0].values = TrackValues::Vec3s(vec![Vec3::splat(f32::NAN); 3]),
                6..=9 => {
                    bad.tracks[0].property = Property::Rotation;
                    bad.tracks[0].interpolation = Interpolation::Step;
                    bad.tracks[0].values = TrackValues::Quats(vec![
                        match mutation {
                            6 => Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
                            7 => Quat::from_xyzw(0.0, 0.0, 0.0, 1e-20),
                            8 => Quat::from_xyzw(0.0, 0.0, 0.0, 1.001),
                            _ => Quat::from_xyzw(f32::INFINITY, 0.0, 0.0, 1.0),
                        };
                        3
                    ]);
                }
                _ => unreachable!(),
            }
            doc.clips.push(bad);
            let data = payload(&doc, false, None);
            assert_eq!(data["clips"][0], good["clips"][0]);
            assert!(
                data["clips"][1].get("locals").is_none(),
                "mutation {mutation}"
            );
            assert!(
                data["clips"][1]["blend_omission"].is_string(),
                "mutation {mutation}"
            );
            assert!(data["clips"][1]["positions"].is_string());
            assert_eq!(data["findings"], good["findings"]);
            assert_eq!(
                payload(&doc, false, Some("a"))["clips"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
        }
    }
    #[test]
    fn sampled_rest_quaternion_norm_admission() {
        for (w, allowed) in [
            (0.0, false),
            (1e-20, false),
            (1.001, false),
            (1.00005, true),
            (f32::INFINITY, false),
        ] {
            let mut doc = document();
            doc.skeleton.bones[0].rest.rotation = Quat::from_xyzw(0.0, 0.0, 0.0, w);
            let data = payload(&doc, false, None);
            assert_eq!(data["clips"][0].get("locals").is_some(), allowed, "w={w}");
        }
    }
    #[test]
    fn aggregate_limits_are_checked_before_value_admission() {
        let mut doc = document();
        // The last complete 40-byte record under 32 MiB, then one more.
        doc.clips[0].tracks[0].times = vec![0.0; MAX_BYTES / 40];
        assert_eq!(preflight(&doc, None), Ok(()));
        doc.clips[0].tracks[0].times.push(0.0);
        assert_eq!(preflight(&doc, None), Err(BUDGET_NOTICE));
        assert_eq!(local_bytes(usize::MAX, 2), None);
        assert_eq!(base64_bytes(usize::MAX), None);
        let mut doc = document();
        doc.clips = vec![doc.clips[0].clone(); MAX_CLIPS];
        assert_eq!(preflight(&doc, None), Ok(()));
        doc.clips.push(doc.clips[0].clone());
        assert_eq!(preflight(&doc, None), Err(BUDGET_NOTICE));
        let mut doc = document();
        doc.clips[0].tracks = vec![doc.clips[0].tracks[0].clone(); MAX_TRACKS];
        assert_eq!(preflight(&doc, None), Ok(()));
        let extra = doc.clips[0].tracks[0].clone();
        doc.clips[0].tracks.push(extra);
        assert_eq!(preflight(&doc, None), Err(BUDGET_NOTICE));
        let mut doc = document();
        doc.skeleton.bones = vec![doc.skeleton.bones[0].clone(); MAX_BONES];
        assert_eq!(preflight(&doc, None), Ok(()));
        doc.skeleton.bones.push(doc.skeleton.bones[0].clone());
        assert!(preflight(&doc, None).is_err());
        doc.skeleton.bones.truncate(2);
        doc.skeleton.bones[0].parent = Some(1);
        let data = payload(&doc, false, None);
        assert!(
            data["blend"]["omission"]
                .as_str()
                .unwrap()
                .contains("parent hierarchy")
        );
        assert!(data["clips"][0]["positions"].is_string());
        assert!(data["clips"][0].get("locals").is_none());
    }
    #[test]
    fn aggregate_over_budget_omits_all_locals_and_keeps_report_success() {
        let mut doc = document();
        // Invalid metadata on the second clip cannot reclaim aggregate budget.
        doc.clips[0].tracks[0].times = vec![0.0; MAX_BYTES / 40 / 2 + 1];
        let mut other = doc.clips[0].clone();
        other.name = "b".into();
        doc.clips.push(other);
        let full = payload(&doc, false, None);
        assert_eq!(full["blend"]["omission"], BUDGET_NOTICE);
        assert_eq!(full["blend"]["raw_bytes"], 0);
        assert!(
            full["clips"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c.get("locals").is_none() && c["positions"].is_string())
        );
        let selected = payload(&doc, false, Some("a"));
        assert_eq!(
            full["clips"][0]["positions"],
            selected["clips"][0]["positions"]
        );
        assert_eq!(full["findings"], selected["findings"]);
    }
}
