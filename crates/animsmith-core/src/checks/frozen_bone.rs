//! `frozen-bone` — a required bone that carries keyframes but whose
//! rotation never exceeds a floor is frozen: a T-posed limb, a
//! wrong-source slice, or a masked-out channel that the presence-only
//! `missing-bones` check would pass. Real motion moves required bones
//! tens of degrees; the default 1° floor catches truly static bones
//! without flagging subtle idle sway.

use super::constant_track::quaternion_angular_delta;
use crate::check::{Check, CheckCtx};
use crate::evaluation::{Applicability, CheckOutput};
use crate::finding::{Finding, Severity};
use crate::metrics::rotation_range_deg;
use crate::model::{Interpolation, Property, Track, TrackValues};
use glam::{DVec4, Quat};

pub const DEFAULT_MIN_ROTATION_DEG: f64 = 1.0;

pub struct FrozenBone;

impl Check for FrozenBone {
    fn id(&self) -> &'static str {
        "frozen-bone"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx
            .clip_expectations()
            .iter()
            .any(|expectations| expectations.animates_bones.is_some())
        {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> Result<CheckOutput, crate::EvaluationError> {
        let mut findings = Vec::new();
        let floor = ctx
            .config
            .check_settings(self.id())
            .min_rotation_deg
            .unwrap_or(DEFAULT_MIN_ROTATION_DEG);

        for (index, clip) in ctx.doc.clips.iter().enumerate() {
            let Some(required) = ctx.expectations(index).animates_bones.as_deref() else {
                continue;
            };
            for bone_name in required {
                let Some(bone_id) = ctx
                    .doc
                    .skeleton
                    .bones
                    .iter()
                    .position(|b| &b.name == bone_name)
                else {
                    continue; // missing-bones owns absent bones
                };
                // Establish motion relative to the first rotation key. A bone
                // with no rotation track at all is also frozen rotation-wise, but that reads as
                // missing-bones territory only if it has no tracks —
                // with translation-only tracks it belongs here.
                let mut has_any_track = false;
                let mut max_deg = 0.0f64;
                let mut inconclusive = None;
                for track in clip.tracks.iter().filter(|t| t.bone == bone_id) {
                    has_any_track = true;
                    match rotation_evidence(track, floor) {
                        Ok(Some(deg)) => max_deg = max_deg.max(deg),
                        Ok(None) => {}
                        Err(reason) => inconclusive = Some(reason),
                    }
                }
                // Any established motion witness suffices. Otherwise an
                // undecided cubic track cannot become a frozen finding.
                if max_deg < floor
                    && let Some(reason) = inconclusive
                {
                    return Err(crate::EvaluationError::Inconclusive {
                        check_id: self.id(),
                        reason: format!("clip {:?}, bone {bone_name:?}: {reason}", clip.name),
                    });
                }
                if has_any_track && max_deg < floor {
                    findings.push(
                        Finding::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "required bone rotates only {max_deg:.2}° over the clip \
                                 (floor {floor:.2}°) — frozen/T-posed limb or a \
                                 wrong-source slice"
                            ),
                        )
                        .clip(&clip.name)
                        .bone(bone_name.clone())
                        .measured(max_deg)
                        .expected(floor),
                    );
                }
            }
        }
        Ok(CheckOutput::from_coverage(findings, Vec::new(), Vec::new()))
    }
}

/// Keyed ranges decide LINEAR/STEP as before. A cubic midpoint is only a
/// positive movement witness, never an estimate of the curve's maximum.
fn rotation_evidence(track: &Track, floor: f64) -> Result<Option<f64>, &'static str> {
    if track.property != Property::Rotation || track.interpolation != Interpolation::CubicSpline {
        return Ok(rotation_range_deg(track));
    }
    let TrackValues::Quats(values) = &track.values else {
        return Err("cubic rotation has no quaternion values");
    };
    let (keys, remainder) = values.as_chunks::<3>();
    if track.times.is_empty()
        || keys.len() != track.times.len()
        || !remainder.is_empty()
        || track.times.iter().any(|time| !time.is_finite())
        || track
            .times
            .windows(2)
            .any(|pair| matches!(pair, [left, right] if left >= right))
        || values.iter().any(|value| !value.is_finite())
    {
        return Err("cubic rotation has invalid key times, values or tangent structure");
    }
    let first = track.key_quat(0).ok_or("cubic rotation has no anchor")?;
    if keys
        .iter()
        .any(|[_, value, _]| quaternion_angular_delta(*value, *value).is_none())
    {
        return Err("cubic rotation has an undefined zero quaternion key");
    }
    let keyed = rotation_range_deg(track).ok_or("cubic rotation has no usable anchor")?;
    if keyed >= floor {
        return Ok(Some(keyed));
    }
    let zero = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
    // Equal nonzero raw values and zero participating tangents give exactly
    // the same quaternion throughout every Hermite segment. Requiring the
    // same sign also rules out interpolation through zero. The first incoming
    // and final outgoing tangent are not part of any segment.
    let flat = keys.iter().all(|[_, value, _]| *value == first)
        && keys.windows(2).all(|pair| matches!(pair, [[_, _, outgoing], [incoming, _, _]] if *outgoing == zero && *incoming == zero));
    if flat {
        return Ok(Some(0.0));
    }
    for (times, keys) in track.times.windows(2).zip(keys.windows(2)) {
        let ([start, end], [[_, left, outgoing], [incoming, right, _]]) = (times, keys) else {
            return Err("cubic rotation segment structure is invalid");
        };
        let dt = f64::from(*end) - f64::from(*start);
        let wide = |q: Quat| DVec4::from_array(q.to_array().map(f64::from));
        // Hermite at u=1/2: h00=h01=1/2, h10=1/8, h11=-1/8.
        // Widen before products, then scale before narrowing to retain finite
        // source quaternions and tangent*time products outside f32's range.
        let midpoint =
            (wide(*left) + wide(*right)) * 0.5 + (wide(*outgoing) - wide(*incoming)) * (dt * 0.125);
        let scale = midpoint.abs().max_element();
        if scale > 0.0 {
            let witness = Quat::from_array((midpoint / scale).to_array().map(|v| v as f32));
            if let Some(degrees) =
                quaternion_angular_delta(first, witness).map(|rad| f64::from(rad.to_degrees()))
                && degrees >= floor
            {
                return Ok(Some(degrees));
            }
        }
    }
    Err(
        "cubic rotation is not a proven flat hold and its keys/midpoints do not establish the configured rotation floor",
    )
}
