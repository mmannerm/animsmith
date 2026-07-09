//! The analytic walk-cycle fixture, shared by this crate's semantic
//! tests and [`animsmith-testkit`](../../animsmith_testkit/index.html)'s
//! committed example assets.
//!
//! A `hips` + left/right-foot rig whose feet swing as antiphase
//! sinusoids, so the loop-seam / gait / root-motion metrics (which
//! FK-sample foot position relative to the hips) have real motion to
//! measure. The bone names are a [`WalkBones`] parameter: `semantic.rs`
//! wires explicit roles over `l_foot`/`r_foot` to unit-test the checks
//! with profile detection bypassed, while testkit uses `foot_l`/`foot_r`
//! so the committed `walk.glb` resolves the `ue-mannequin` profile
//! end-to-end. `periods` and `stride` are parameters for the same reason
//! — one closed loop vs. a popped seam, a real stride vs. a tiny one.
//!
//! Behind the `fixtures` feature: testkit enables it, and this crate's
//! own tests reach it through a self dev-dependency that turns the
//! feature on for the test build.

use crate::model::*;
use crate::profile::{ResolvedRoles, Role};
use glam::Vec3;
use std::f64::consts::TAU;

/// Keyframe count: 32 intervals over the 1 s clip.
pub const WALK_KEYS: usize = 33;
/// Vertical foot swing amplitude, metres.
pub const WALK_FOOT_AMPLITUDE: f32 = 0.05;
/// Default fore/aft foot swing (stride), metres. Some tests vary it to
/// exercise the loop-seam stride floor.
pub const WALK_STRIDE: f32 = 0.15;

/// The bone names for the three-bone walk rig — a `hips` root with a left
/// and a right foot. A parameter so one consumer can pick
/// profile-resolving names and another explicit-role test names, off the
/// single skeleton shape.
pub struct WalkBones {
    /// Name of the pelvis/hips root bone.
    pub hips: &'static str,
    /// Name of the left-foot bone.
    pub left_foot: &'static str,
    /// Name of the right-foot bone.
    pub right_foot: &'static str,
}

impl WalkBones {
    /// The `hips` + `left_foot` / `right_foot` skeleton: a pelvis at
    /// `y = 1` with two feet a metre below it, splayed ±0.1 in X.
    pub fn skeleton(&self) -> Skeleton {
        let foot = |name: &str, x: f32| Bone {
            name: name.into(),
            parent: Some(0),
            rest: Transform {
                translation: Vec3::new(x, -1.0, 0.0),
                ..Transform::IDENTITY
            },
            inverse_bind: None,
        };
        Skeleton {
            bones: vec![
                Bone {
                    name: self.hips.into(),
                    parent: None,
                    rest: Transform {
                        translation: Vec3::new(0.0, 1.0, 0.0),
                        ..Transform::IDENTITY
                    },
                    inverse_bind: None,
                },
                foot(self.left_foot, 0.1),
                foot(self.right_foot, -0.1),
            ],
        }
    }

    /// An explicit hips/left-foot/right-foot role map over `skel` — the
    /// profile-bypass path a semantic test uses to drive the checks
    /// directly, without relying on profile detection.
    pub fn roles(&self, skel: &Skeleton) -> ResolvedRoles {
        ResolvedRoles::from_names(
            skel,
            [
                (Role::Hips, self.hips.to_string()),
                (Role::LeftFoot, self.left_foot.to_string()),
                (Role::RightFoot, self.right_foot.to_string()),
            ],
        )
    }
}

/// One foot's translation track: an antiphase vertical + fore/aft
/// sinusoid over `periods` cycles with the given `stride`. `periods = 1.0`
/// closes the loop exactly (seam ≈ 0); a non-integer count leaves the feet
/// away from their first-frame pose — a popped seam.
pub fn foot_track(bone: BoneId, rest: Vec3, sign: f32, periods: f64, stride: f32) -> Track {
    let times: Vec<f32> = (0..WALK_KEYS)
        .map(|k| k as f32 / (WALK_KEYS - 1) as f32)
        .collect();
    let values: Vec<Vec3> = (0..WALK_KEYS)
        .map(|k| {
            let theta = periods * TAU * k as f64 / (WALK_KEYS - 1) as f64;
            // `libm::sin` (pure Rust) is bit-identical across platforms,
            // unlike `f32::sin`, so the committed asset regenerates
            // byte-for-byte on Linux / macOS / Windows.
            let swing = libm::sin(theta) as f32;
            rest + Vec3::new(
                0.0,
                sign * WALK_FOOT_AMPLITUDE * swing,
                sign * stride * swing,
            )
        })
        .collect();
    Track {
        bone,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times,
        values: TrackValues::Vec3s(values),
    }
}

/// The analytic walk clip over `bones`: antiphase left/right foot tracks
/// running `periods` cycles at `stride`, in a clip named `clip`.
pub fn walk_doc(bones: &WalkBones, clip: &str, periods: f64, stride: f32) -> Document {
    let skeleton = bones.skeleton();
    let tracks = vec![
        foot_track(1, skeleton.bones[1].rest.translation, 1.0, periods, stride),
        foot_track(2, skeleton.bones[2].rest.translation, -1.0, periods, stride),
    ];
    Document {
        skeleton,
        clips: vec![Clip {
            name: clip.into(),
            duration_s: 1.0,
            tracks,
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}
