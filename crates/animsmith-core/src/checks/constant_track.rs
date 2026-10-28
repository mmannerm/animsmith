//! `constant-track` — a multi-key track whose values never move is
//! export bloat (unbaked rig channels, "key everything" exports). Note
//! severity: harmless at runtime, wasteful on disk and in blends.
//! "Unexpectedly constant" (a bone that *should* move) is the
//! `frozen-bone` check and needs per-clip expectations (M1).

use super::tracks;
use super::vec3_trajectory::analyze;
use crate::check::{Check, CheckCtx};
use crate::evaluation::CheckOutput;
use crate::finding::{Finding, Severity};
use crate::model::TrackValues;
use glam::Quat;

/// Positional/scale spread (in source units) below which a track is
/// constant.
pub const VEC3_TOLERANCE: f32 = 1e-4;

/// Rotation deviation below which a track is constant (radians;
/// ~0.06°).
pub const QUAT_TOLERANCE_RAD: f32 = 1e-3;

pub struct ConstantTrack;

impl Check for ConstantTrack {
    fn id(&self) -> &'static str {
        "constant-track"
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let doc = ctx.doc;
        for (clip, bone, track) in tracks(doc) {
            if track.key_count() < 2 {
                continue; // a single-key track is a compact pin, not bloat
            }
            if is_constant_track(track) {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Note,
                        format!(
                            "{} track has {} keys but never moves — export bloat",
                            track.property.as_str(),
                            track.key_count()
                        ),
                    )
                    .clip(clip)
                    .bone(bone),
                );
            }
        }
        CheckOutput::from_coverage(findings, Vec::new(), Vec::new())
    }
}

/// Whether a multi-key track is constant under this check's exact,
/// interpolation-aware tolerances. Malformed or non-finite tracks are never
/// candidates: source-data checks own those diagnostics.
pub(crate) fn is_constant_track(track: &crate::model::Track) -> bool {
    if track.key_count() < 2 {
        return false;
    }
    match &track.values {
        TrackValues::Vec3s(_) => {
            analyze(track).is_some_and(|trajectory| !trajectory.varies(VEC3_TOLERANCE))
        }
        TrackValues::Quats(_) => {
            let Some(first) = track.key_quat(0) else {
                return false;
            };
            (1..track.key_count()).all(|k| {
                track.key_quat(k).is_some_and(|q| {
                    // Cubic interpolation uses authored components, without
                    // shortest-path sign correction. Opposite hemispheres can
                    // create a large interior rotation even with zero tangents.
                    (track.interpolation != crate::model::Interpolation::CubicSpline
                        || first.as_dquat().dot(q.as_dquat()) > 0.0)
                        && quaternion_angular_delta(first, q)
                            .is_some_and(|delta| delta <= QUAT_TOLERANCE_RAD)
                })
            }) && (track.interpolation != crate::model::Interpolation::CubicSpline
                || cubic_quaternion_tangents_are_zero(track))
        }
    }
}

/// Stable shortest-path angular distance for equivalent quaternion signs.
///
/// Using `acos(dot)` loses too much precision around this check's milliradian
/// threshold on some f32 targets. The relative quaternion's vector magnitude
/// keeps that small-angle evidence for the `atan2` formulation.
/// Widen before squaring or normalizing: every finite f32 component, including
/// a subnormal, has a representable square in f64. Only the bounded final angle
/// is narrowed back to the caller's existing f32 tolerance domain.
pub(crate) fn quaternion_angular_delta(first: Quat, other: Quat) -> Option<f32> {
    let first = first.as_dquat();
    let other = other.as_dquat();
    if !first.is_finite()
        || !other.is_finite()
        || first.length_squared() == 0.0
        || other.length_squared() == 0.0
    {
        return None;
    }
    let delta = first.normalize().conjugate() * other.normalize();
    let [x, y, z, w] = delta.to_array();
    let sin_half_angle = glam::DVec3::new(x, y, z).length();
    let angle = (2.0 * sin_half_angle.atan2(w.abs())) as f32;
    angle.is_finite().then_some(angle)
}

/// Cubic quaternion key values can agree while their tangents still generate
/// a changing component-wise Hermite curve. Treat that representation as
/// redundant only for a genuinely flat hold.
pub(crate) fn cubic_quaternion_tangents_are_zero(track: &crate::model::Track) -> bool {
    let TrackValues::Quats(values) = &track.values else {
        return false;
    };
    let zero = glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
    (0..track.key_count())
        .all(|key| values.get(3 * key) == Some(&zero) && values.get(3 * key + 2) == Some(&zero))
}

#[cfg(test)]
mod tests {
    use super::{QUAT_TOLERANCE_RAD, is_constant_track, quaternion_angular_delta};
    use crate::model::{Interpolation, Property, Track, TrackValues};
    use glam::Quat;

    #[test]
    fn milliradian_distance_retains_small_angle_precision() {
        let step = Quat::from_rotation_z(QUAT_TOLERANCE_RAD * 0.6);

        assert!(
            quaternion_angular_delta(Quat::IDENTITY, step).expect("finite step")
                <= QUAT_TOLERANCE_RAD
        );
        assert!(
            quaternion_angular_delta(Quat::IDENTITY, step * step).expect("finite cumulative step")
                > QUAT_TOLERANCE_RAD
        );
        assert_eq!(
            quaternion_angular_delta(Quat::IDENTITY, -(step * step)),
            quaternion_angular_delta(Quat::IDENTITY, step * step)
        );
    }

    #[test]
    fn known_rotation_angle_survives_the_finite_f32_magnitude_range() {
        for scale in [1.0, 1e20, 1e-30, f32::from_bits(1), f32::MAX] {
            let first = Quat::from_xyzw(scale, scale, scale, scale);
            let other = Quat::from_xyzw(scale, -scale, -scale, -scale);
            // Unit endpoints are [1,1,1,1]/2 and [1,-1,-1,-1]/2.
            // Their dot is -1/2: the shortest orientation difference is 120°.
            let angle = quaternion_angular_delta(first, other).expect("finite nonzero keys");
            assert!(
                (angle - 2.0 * std::f32::consts::PI / 3.0).abs() < 1e-6,
                "{scale}: {angle}"
            );
            assert_eq!(quaternion_angular_delta(first, first), Some(0.0));
            assert_eq!(quaternion_angular_delta(first, -first), Some(0.0));
        }
    }

    #[test]
    fn milliradian_threshold_is_independent_of_key_magnitude() {
        for scale in [1.0, 1e20, 1e-30] {
            for (slope, constant) in [(0.000_499, true), (0.000_501, false)] {
                // A single-axis [0,0,slope,1] rotation has angle 2*atan(slope):
                // these straddle 1e-3 radians at approximately 0.000998/0.001002.
                let first = Quat::from_xyzw(0.0, 0.0, 0.0, scale);
                let other = Quat::from_xyzw(0.0, 0.0, slope * scale, scale);
                let angle = quaternion_angular_delta(first, other).unwrap();
                assert_eq!(
                    angle <= QUAT_TOLERANCE_RAD,
                    constant,
                    "{scale}, {slope}: {angle}"
                );
            }
        }
    }

    #[test]
    fn zero_and_nonfinite_rotations_are_not_angular_evidence() {
        for invalid in [
            Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
            Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0),
            Quat::from_xyzw(0.0, f32::INFINITY, 0.0, 1.0),
        ] {
            assert_eq!(quaternion_angular_delta(invalid, Quat::IDENTITY), None);
            assert_eq!(quaternion_angular_delta(Quat::IDENTITY, invalid), None);
        }
    }

    #[test]
    fn large_cubic_keys_still_require_one_hemisphere_and_flat_tangents() {
        let zero = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
        let first = Quat::from_xyzw(2e34, 0.0, 0.0, 1e38);
        let other = Quat::from_xyzw(-2e34, 0.0, 0.0, 1e38);
        // The orientation difference is 4*atan(2e-4), about 0.0008 radians.
        // The positive raw dot includes opposing products too large for f32.
        let track = |end, tangent| Track {
            bone: 0,
            property: Property::Rotation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![zero, first, tangent, zero, end, zero]),
        };
        assert!(is_constant_track(&track(other, zero)));
        assert!(!is_constant_track(&track(-other, zero)));
        assert!(!is_constant_track(&track(
            other,
            Quat::from_xyzw(1e-30, 0.0, 0.0, 0.0)
        )));
    }
}
