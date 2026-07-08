//! Fixture builders shared by the workspace's tests and the example
//! asset generator. The builders construct [`animsmith_core`] model
//! types and do no I/O; [`write_example_assets`] adds the committed
//! example assets' filename↔document wiring, taking the writer as an
//! argument so this crate needs no glTF-writer dependency.
//!
//! The single two-bone rotation clip built by [`two_bone_rotation_doc`]
//! is the common shape behind `crates/animsmith-gltf/tests/fix.rs`,
//! `crates/animsmith/tests/cli_contract.rs`, and the committed example
//! assets.

use animsmith_core::model::*;
use glam::{Quat, Vec3};
use std::f64::consts::TAU;
use std::path::Path;

/// Keyframe times every rotation fixture shares (five keys over 1 s).
const ROTATION_TIMES: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

/// Unit Y-rotation keys for the given angles, in radians. Pass literal
/// angles (not a computed ramp) so callers control the exact `f32`
/// values — the generated glTF bytes depend on them.
pub fn quats_from_angles(angles: &[f32]) -> Vec<Quat> {
    angles.iter().map(|&a| Quat::from_rotation_y(a)).collect()
}

/// Scale a quaternion's components off the unit sphere. The result
/// represents the same rotation once normalized, so it is a lossless
/// `quat-norm` defect.
pub fn scaled_quat(q: Quat, scale: f32) -> Quat {
    let [x, y, z, w] = q.to_array();
    Quat::from_xyzw(x * scale, y * scale, z * scale, w * scale)
}

/// The two-bone `root -> spine` skeleton the rotation fixtures animate.
fn two_bone_skeleton() -> Skeleton {
    Skeleton {
        bones: vec![
            Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "spine".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(0.0, 0.5, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ],
    }
}

/// A two-bone `root -> spine` skeleton with one 1 s rotation clip on the
/// spine. `with_translation` adds a root translation track (some fix tests
/// assert it survives a repair byte-identically); the rotation track is
/// emitted first either way.
pub fn two_bone_rotation_doc(clip: &str, quats: Vec<Quat>, with_translation: bool) -> Document {
    let mut tracks = vec![Track {
        bone: 1,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: ROTATION_TIMES.to_vec(),
        values: TrackValues::Quats(quats),
    }];
    if with_translation {
        tracks.push(Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::new(0.0, 0.0, 2.0)]),
        });
    }
    Document {
        skeleton: two_bone_skeleton(),
        clips: vec![Clip {
            name: clip.into(),
            duration_s: 1.0,
            tracks,
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}

/// Angles behind the committed example clip (`examples/assets/clip.glb`).
const EXAMPLE_ANGLES: [f32; 5] = [0.0, 0.1, 0.2, 0.3, 0.4];

/// The clean committed example clip, `examples/assets/clip.glb`: a
/// gentle `swing` with no defects.
fn example_clean_doc() -> Document {
    two_bone_rotation_doc("swing", quats_from_angles(&EXAMPLE_ANGLES), false)
}

/// The dirty committed example clip, `examples/assets/clip-dirty.glb`:
/// the clean clip with two repairable defects — key 2 scaled off unit
/// (`quat-norm`) and key 3 sign-flipped (`quat-flip`).
fn example_dirty_doc() -> Document {
    let mut quats = quats_from_angles(&EXAMPLE_ANGLES);
    quats[2] = scaled_quat(quats[2], 1.05);
    quats[3] = -quats[3];
    two_bone_rotation_doc("swing", quats, false)
}

// --- Analytic walk rig (semantic checks) -----------------------------
//
// A hips + left/right-foot rig whose feet swing as antiphase sinusoids,
// so the loop-seam / gait / root-motion metrics (which FK-sample foot
// position relative to the hips) have real motion to measure.

const WALK_KEYS: usize = 33; // 32 intervals over 1 s
const WALK_FOOT_AMPLITUDE: f32 = 0.05; // vertical foot swing, metres
const WALK_STRIDE: f32 = 0.15; // fore/aft foot swing, metres

/// A `pelvis` + `foot_l` / `foot_r` skeleton. The bone names resolve the
/// built-in `ue-mannequin` profile, so semantic checks fire under
/// `profile = "auto"` with no inline role map.
fn walk_skeleton() -> Skeleton {
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
                name: "pelvis".into(),
                parent: None,
                rest: Transform {
                    translation: Vec3::new(0.0, 1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            foot("foot_l", 0.1),
            foot("foot_r", -0.1),
        ],
    }
}

/// One foot's translation track: an antiphase vertical + fore/aft
/// sinusoid over `periods` cycles. `periods = 1.0` closes the loop
/// exactly (seam ≈ 0); a non-integer count leaves the feet away from
/// their first-frame pose — a popped seam.
fn foot_track(bone: BoneId, rest: Vec3, sign: f32, periods: f64) -> Track {
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
                sign * WALK_STRIDE * swing,
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

fn walk_doc(clip: &str, periods: f64) -> Document {
    let skeleton = walk_skeleton();
    let tracks = vec![
        foot_track(1, skeleton.bones[1].rest.translation, 1.0, periods),
        foot_track(2, skeleton.bones[2].rest.translation, -1.0, periods),
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

/// The clean committed walk clip (`examples/assets/walk.glb`): a 1 s
/// cycle that closes exactly, so the loop seam is ≈ 0.
fn example_walk_doc() -> Document {
    walk_doc("walk", 1.0)
}

/// The popped-seam walk clip (`examples/assets/walk-dirty.glb`): the same
/// motion cut at ¾ of a cycle, so the feet never return to their
/// first-frame pose and the loop seam pops.
fn example_walk_dirty_doc() -> Document {
    walk_doc("walk", 0.75)
}

/// The committed example assets under `examples/assets/`, as
/// `(filename, document)` pairs — the single filename↔document wiring.
fn example_assets() -> [(&'static str, Document); 4] {
    [
        ("clip.glb", example_clean_doc()),
        ("clip-dirty.glb", example_dirty_doc()),
        ("walk.glb", example_walk_doc()),
        ("walk-dirty.glb", example_walk_dirty_doc()),
    ]
}

/// Write every committed example asset into `dir`, emitting each
/// document through the injected `write` (so this crate stays free of a
/// glTF-writer dependency). Both the `gen_example_assets` example and the
/// drift-guard test drive their writes through this one function, so a
/// wrong filename, a dropped asset, or a swapped clean/dirty document is
/// exercised — and caught — by the test rather than only surfacing when a
/// human reruns the generator.
pub fn write_example_assets<E>(
    dir: &Path,
    mut write: impl FnMut(&Document, &Path) -> Result<(), E>,
) -> Result<(), E> {
    for (name, doc) in example_assets() {
        write(&doc, &dir.join(name))?;
    }
    Ok(())
}
