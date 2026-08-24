//! The built-in check catalog. Each module is one check with its
//! defaults documented on the type. See DESIGN.md §6 for the tiers.

pub mod bind_pose;
pub mod constant_nonunit_scale;
pub mod constant_track;
pub mod duplicate_loop_endpoint;
pub mod duration_sanity;
pub mod foot_slide;
pub mod fps;
pub mod frozen_bone;
pub mod gait_group;
pub mod in_place;
pub mod loop_closure;
pub mod loop_seam;
pub mod loop_seam_rot;
pub mod loop_seam_vel;
pub mod missing_bones;
pub mod nan;
pub mod non_uniform_scale;
pub mod quat_flip;
pub mod quat_norm;
pub mod required_bones;
pub mod rest_world_scale;
pub mod root_motion_speed;
pub mod scale_keys;
pub mod sync_group;
pub mod time_complement;
pub mod time_monotonic;

mod vec3_trajectory;

use crate::evaluation::{CoverageGap, CoverageGapCode};
use crate::metrics::GaitPhaseOutcome;
use crate::model::{Document, Track};
use crate::profile::{ResolvedRoles, Role};

/// Identifies the check-specific wording for a gait-phase coverage gap.
#[derive(Clone, Copy)]
pub(crate) enum GaitPhaseGapContext {
    GaitGroup,
    TimeComplement,
}

/// Map every gait-phase outcome to its check-specific coverage gap.
///
/// A measured phase is not a gap; the other outcomes retain the existing
/// diagnostic wording while sharing the coverage classification authority.
pub(crate) fn gait_phase_gap(
    outcome: GaitPhaseOutcome,
    context: GaitPhaseGapContext,
) -> Option<CoverageGap> {
    let message = match (outcome, context) {
        (GaitPhaseOutcome::Measured(_), _) => return None,
        (GaitPhaseOutcome::MissingBilateralFootRoles, GaitPhaseGapContext::GaitGroup) => {
            "gait phase has no bilateral foot-role subject"
        }
        (GaitPhaseOutcome::MissingBilateralFootRoles, GaitPhaseGapContext::TimeComplement) => {
            "gait phase has no bilateral foot-role subject for time-complement comparison"
        }
        (GaitPhaseOutcome::NoFootHeightSwing, GaitPhaseGapContext::GaitGroup) => {
            "gait phase has no left/right foot-height swing"
        }
        (GaitPhaseOutcome::NoFootHeightSwing, GaitPhaseGapContext::TimeComplement) => {
            "gait phase has no left/right foot-height swing for time-complement comparison"
        }
        (GaitPhaseOutcome::Unavailable, GaitPhaseGapContext::GaitGroup) => {
            "gait phase could not be fitted from the sampled cycle"
        }
        (GaitPhaseOutcome::Unavailable, GaitPhaseGapContext::TimeComplement) => {
            "gait phase could not be fitted for time-complement comparison"
        }
    };
    let code = if matches!(outcome, GaitPhaseOutcome::MissingBilateralFootRoles) {
        CoverageGapCode::ROLES_UNRESOLVED
    } else {
        CoverageGapCode::MEASUREMENT_UNAVAILABLE
    };
    Some(CoverageGap::new(code, message))
}

/// Whether an `f32`-derived metric exceeds a user-facing `f64` cap.
///
/// Source transforms and sampled poses are `f32`, so decimal values such as
/// `0.1` can round slightly upward when promoted into measurement output. The
/// comparison quantizes both sides to the evidence precision so a value
/// authored exactly at an inclusive cap does not become a false positive.
pub(crate) fn exceeds_f32_cap(measured: f64, cap: f64) -> bool {
    (measured as f32) > (cap as f32)
}

/// Return the typed prerequisite gap for root-motion work, if any.
pub(crate) fn root_motion_gap(roles: &ResolvedRoles) -> Option<CoverageGap> {
    if roles.get(Role::Root).is_some() || roles.get(Role::Hips).is_some() {
        None
    } else {
        Some(CoverageGap::new(
            CoverageGapCode::ROLES_UNRESOLVED,
            format!(
                "root/hips role not resolved (rig profile '{}')",
                roles.profile
            ),
        ))
    }
}

/// Return the typed prerequisite gap for gait work, if any.
pub(crate) fn gait_gap(roles: &ResolvedRoles) -> Option<CoverageGap> {
    let has_foot = [
        Role::LeftFoot,
        Role::LeftToe,
        Role::RightFoot,
        Role::RightToe,
    ]
    .iter()
    .any(|&r| roles.get(r).is_some());
    if roles.get(Role::Hips).is_some() && has_foot {
        None
    } else {
        Some(CoverageGap::new(
            CoverageGapCode::ROLES_UNRESOLVED,
            format!(
                "hips/foot roles not resolved (rig profile '{}') — needs hips and at least one foot role",
                roles.profile
            ),
        ))
    }
}

/// Iterate `(clip name, bone name, track)` across a document.
pub(crate) fn tracks(doc: &Document) -> impl Iterator<Item = (&str, &str, &Track)> {
    doc.clips.iter().flat_map(move |clip| {
        clip.tracks.iter().map(move |track| {
            let bone = doc
                .skeleton
                .bones
                .get(track.bone)
                .map(|b| b.name.as_str())
                .unwrap_or("<unknown>");
            (clip.name.as_str(), bone, track)
        })
    })
}

#[cfg(test)]
mod gait_phase_gap_tests {
    use super::{GaitPhaseGapContext, gait_phase_gap};
    use crate::evaluation::CoverageGapCode;
    use crate::metrics::GaitPhaseOutcome;

    #[test]
    fn defensive_unavailable_outcome_keeps_each_check_specific_measurement_gap() {
        for (context, message) in [
            (
                GaitPhaseGapContext::GaitGroup,
                "gait phase could not be fitted from the sampled cycle",
            ),
            (
                GaitPhaseGapContext::TimeComplement,
                "gait phase could not be fitted for time-complement comparison",
            ),
        ] {
            let gap = gait_phase_gap(GaitPhaseOutcome::Unavailable, context)
                .expect("unavailable phase is a defensive coverage gap");
            assert_eq!(gap.code, CoverageGapCode::MEASUREMENT_UNAVAILABLE);
            assert_eq!(gap.message, message);
        }
    }
}
