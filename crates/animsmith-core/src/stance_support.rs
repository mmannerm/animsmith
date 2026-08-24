//! Shared V1 sampled stance-support classification.
//!
//! This module owns the foot-first/toe-fallback selection and the sampled
//! model-space height predicate shared by contact-oriented consumers. It is a
//! sampled observation, not a physical-contact or gameplay claim.

use crate::model::BoneId;
use crate::profile::{ResolvedRoles, Role};
use crate::sample::PoseGrid;

/// One independently resolved side of a bilateral stance observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StanceSideV1 {
    /// The character's left side.
    Left,
    /// The character's right side.
    Right,
}

impl StanceSideV1 {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    fn resolved_role(self, roles: &ResolvedRoles) -> Option<(Role, BoneId)> {
        let preferred = match self {
            Self::Left => [Role::LeftFoot, Role::LeftToe],
            Self::Right => [Role::RightFoot, Role::RightToe],
        };
        preferred
            .into_iter()
            .find_map(|role| roles.get(role).map(|bone| (role, bone)))
    }
}

/// One maximal V1 support run, expressed as inclusive sampled-frame indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StanceSupportRunV1 {
    /// First supporting frame in the run.
    pub start_frame: usize,
    /// Last supporting frame in the run.
    pub end_frame: usize,
}

/// Sampled support evidence for one selected foot or toe role.
///
/// The classifier intentionally preserves the legacy `foot-slide` reduction
/// and comparison semantics, including their behavior for non-finite sampled
/// heights. Strict consumers that need finite evidence must validate it at
/// their own boundary before using this observation.
#[derive(Debug)]
pub struct ResolvedStanceSupportV1<'a> {
    grid: &'a PoseGrid,
    role: Role,
    bone: BoneId,
    ground_y_m: f64,
    contact_height_m: f64,
}

/// Resolve one side and classify its sampled stance support.
///
/// The selected role is Foot first and Toe only as a same-side fallback. The
/// ground reference is the selected role's own minimum model-space Y over the
/// complete grid; sides never share a ground reference. The reduction and
/// comparison preserve the existing `foot-slide` behavior exactly.
pub fn resolve_stance_support_v1<'a>(
    grid: &'a PoseGrid,
    roles: &ResolvedRoles,
    side: StanceSideV1,
    contact_height_m: f64,
) -> Option<ResolvedStanceSupportV1<'a>> {
    let (role, bone) = side.resolved_role(roles)?;
    let ground_y_m = (0..grid.frame_count())
        .map(|frame| grid.model_position(frame, bone).y as f64)
        .fold(f64::MAX, f64::min);
    Some(ResolvedStanceSupportV1 {
        grid,
        role,
        bone,
        ground_y_m,
        contact_height_m,
    })
}

impl ResolvedStanceSupportV1<'_> {
    /// The Foot or Toe role selected for this side.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// The selected role's skeleton bone index.
    pub const fn bone(&self) -> BoneId {
        self.bone
    }

    /// Iterate every existing `foot-slide` eligible adjacent-pair endpoint.
    ///
    /// An emitted frame `f` means both `f - 1` and `f` satisfy the V1 support
    /// predicate. Frames are yielded in strictly increasing order; no loop
    /// seam pair is introduced.
    pub fn supported_adjacent_frames(&self) -> impl Iterator<Item = usize> + '_ {
        (1..self.grid.frame_count())
            .filter(|&frame| self.is_support_frame(frame - 1) && self.is_support_frame(frame))
    }

    /// Iterate maximal contiguous supporting runs that contain at least two
    /// sampled frames.
    ///
    /// Singleton supporting samples are deliberately omitted, matching the
    /// adjacent-pair prerequisite consumed by [`Self::supported_adjacent_frames`].
    pub fn retained_runs(&self) -> impl Iterator<Item = StanceSupportRunV1> + '_ {
        let mut next_frame = 0;
        std::iter::from_fn(move || {
            loop {
                while next_frame < self.grid.frame_count() && !self.is_support_frame(next_frame) {
                    next_frame += 1;
                }
                if next_frame == self.grid.frame_count() {
                    return None;
                }
                let start_frame = next_frame;
                while next_frame < self.grid.frame_count() && self.is_support_frame(next_frame) {
                    next_frame += 1;
                }
                let end_frame = next_frame - 1;
                if end_frame > start_frame {
                    return Some(StanceSupportRunV1 {
                        start_frame,
                        end_frame,
                    });
                }
            }
        })
    }

    fn is_support_frame(&self, frame: usize) -> bool {
        // Do not simplify this to `height <= ...`: the established FootSlide
        // predicate deliberately treats NaN differently. The `f64::MAX`
        // ground seed above likewise differs from an infinity seed for +∞.
        let above_threshold = (self.grid.model_position(frame, self.bone).y as f64)
            > self.ground_y_m + self.contact_height_m;
        !above_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Bone, Clip, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };
    use crate::sample::sample_clip;
    use glam::Vec3;

    fn skeleton(names: &[(&str, f32)]) -> Skeleton {
        Skeleton {
            bones: names
                .iter()
                .map(|(name, y)| Bone {
                    name: (*name).into(),
                    parent: None,
                    rest: Transform {
                        translation: Vec3::new(0.0, *y, 0.0),
                        ..Transform::IDENTITY
                    },
                    inverse_bind: None,
                })
                .collect(),
        }
    }

    fn grid(skeleton: Skeleton, tracks: Vec<Track>) -> (Skeleton, PoseGrid) {
        let clip = Clip {
            name: "stance".into(),
            duration_s: 1.0,
            tracks,
        };
        let grid = sample_clip(&skeleton, &clip, 3);
        (skeleton, grid)
    }

    fn y_track(bone: BoneId, values: Vec<f32>) -> Track {
        Track {
            bone,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 0.5, 1.0],
            values: TrackValues::Vec3s(
                values.into_iter().map(|y| Vec3::new(0.0, y, 0.0)).collect(),
            ),
        }
    }

    #[test]
    fn each_side_uses_its_own_model_y_minimum() {
        let (skeleton, grid) = grid(
            skeleton(&[("left", 0.0), ("right", 10.0)]),
            vec![
                y_track(0, vec![0.0, 0.5, 0.0]),
                y_track(1, vec![10.0, 10.5, 10.0]),
            ],
        );
        let roles = ResolvedRoles::from_names(
            &skeleton,
            [
                (Role::LeftFoot, "left".to_string()),
                (Role::RightFoot, "right".to_string()),
            ],
        );

        assert_eq!(grid.model_position(0, 0).y, 0.0);
        assert_eq!(grid.model_position(0, 1).y, 10.0);
        let left = resolve_stance_support_v1(&grid, &roles, StanceSideV1::Left, 0.5).unwrap();
        let right = resolve_stance_support_v1(&grid, &roles, StanceSideV1::Right, 0.5).unwrap();
        assert_eq!(left.supported_adjacent_frames().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(
            right.supported_adjacent_frames().collect::<Vec<_>>(),
            [1, 2]
        );
    }

    #[test]
    fn foot_and_toe_selection_is_independent_per_side() {
        let (skeleton, grid) = grid(
            skeleton(&[("left-foot", 0.0), ("left-toe", 0.0), ("right-toe", 0.0)]),
            vec![y_track(0, vec![0.0, 0.0, 0.0])],
        );
        let roles = ResolvedRoles::from_names(
            &skeleton,
            [
                (Role::LeftFoot, "left-foot".to_string()),
                (Role::LeftToe, "left-toe".to_string()),
                (Role::RightToe, "right-toe".to_string()),
            ],
        );

        let left = resolve_stance_support_v1(&grid, &roles, StanceSideV1::Left, 0.0).unwrap();
        let right = resolve_stance_support_v1(&grid, &roles, StanceSideV1::Right, 0.0).unwrap();
        assert_eq!((left.role(), left.bone()), (Role::LeftFoot, 0));
        assert_eq!((right.role(), right.bone()), (Role::RightToe, 2));
    }

    #[test]
    fn retained_runs_are_maximal_and_match_adjacent_pairs() {
        let skeleton = skeleton(&[("left", 0.0)]);
        let clip = Clip {
            name: "stance".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: (0..8).map(|index| index as f32 / 7.0).collect(),
                values: TrackValues::Vec3s(
                    [0.0, 0.0, 0.1, 0.0, 0.1, 0.0, 0.0, 0.0]
                        .into_iter()
                        .map(|y| Vec3::new(0.0, y, 0.0))
                        .collect(),
                ),
            }],
        };
        let grid = sample_clip(&skeleton, &clip, 8);
        let roles = ResolvedRoles::from_names(&skeleton, [(Role::LeftFoot, "left".to_string())]);
        let support = resolve_stance_support_v1(&grid, &roles, StanceSideV1::Left, 0.0).unwrap();

        assert_eq!(
            support.supported_adjacent_frames().collect::<Vec<_>>(),
            [1, 6, 7]
        );
        assert_eq!(
            support.retained_runs().collect::<Vec<_>>(),
            [
                StanceSupportRunV1 {
                    start_frame: 0,
                    end_frame: 1,
                },
                StanceSupportRunV1 {
                    start_frame: 5,
                    end_frame: 7,
                },
            ]
        );
    }

    #[test]
    fn non_finite_samples_keep_the_legacy_predicate() {
        for (values, expected_endpoints) in [
            (vec![f32::NAN, 0.0, 0.0], vec![1, 2]),
            (vec![f32::INFINITY; 3], vec![]),
        ] {
            let (skeleton, grid) = grid(skeleton(&[("left", 0.0)]), vec![y_track(0, values)]);
            let roles =
                ResolvedRoles::from_names(&skeleton, [(Role::LeftFoot, "left".to_string())]);
            let support =
                resolve_stance_support_v1(&grid, &roles, StanceSideV1::Left, 0.0).unwrap();
            assert_eq!(
                support.supported_adjacent_frames().collect::<Vec<_>>(),
                expected_endpoints
            );
        }
    }
}
