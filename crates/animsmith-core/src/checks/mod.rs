//! The built-in check catalog. Each module is one check with its
//! defaults documented on the type. See DESIGN.md §6 for the tiers.

pub mod bind_pose;
pub mod constant_track;
pub mod duration_sanity;
pub mod foot_slide;
pub mod fps;
pub mod frozen_bone;
pub mod gait_group;
pub mod in_place;
pub mod loop_seam;
pub mod missing_bones;
pub mod nan;
pub mod quat_flip;
pub mod quat_norm;
pub mod root_motion_speed;
pub mod scale_keys;
pub mod time_monotonic;

use crate::check::Readiness;
use crate::model::{Document, Track};
use crate::profile::{ResolvedRoles, Role};

/// Roles a locomotion check needs to measure root travel: a `root`
/// bone, or `hips` as a fallback. Returns [`Readiness::Ready`] when
/// resolved, otherwise a skip-note reason.
pub(crate) fn root_motion_readiness(roles: &ResolvedRoles) -> Readiness {
    if roles.get(Role::Root).is_some() || roles.get(Role::Hips).is_some() {
        Readiness::Ready
    } else {
        Readiness::Skipped(format!(
            "root/hips role not resolved (rig profile '{}')",
            roles.profile
        ))
    }
}

/// Roles a gait check needs: `hips` plus at least one foot/toe.
pub(crate) fn gait_readiness(roles: &ResolvedRoles) -> Readiness {
    let has_foot = [
        Role::LeftFoot,
        Role::LeftToe,
        Role::RightFoot,
        Role::RightToe,
    ]
    .iter()
    .any(|&r| roles.get(r).is_some());
    if roles.get(Role::Hips).is_some() && has_foot {
        Readiness::Ready
    } else {
        Readiness::Skipped(format!(
            "hips/foot roles not resolved (rig profile '{}') — needs hips and at least one foot role",
            roles.profile
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
