//! The built-in check catalog. Each module is one check with its
//! defaults documented on the type. See DESIGN.md §6 for the tiers.

pub mod constant_track;
pub mod duration_sanity;
pub mod nan;
pub mod quat_flip;
pub mod quat_norm;
pub mod scale_keys;
pub mod time_monotonic;

use crate::model::{Document, Track};

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
