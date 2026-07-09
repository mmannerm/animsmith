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
use std::path::Path;

/// Keyframe times every rotation fixture shares (five keys over 1 s).
const ROTATION_TIMES: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

/// Unit Y-rotation keys for the given angles, in radians. Pass literal
/// angles (not a computed ramp) so callers control the exact `f32`
/// values — the generated glTF bytes depend on them. Each key is built
/// via [`quat_from_rotation_y`], whose `libm` trig keeps the committed
/// example assets byte-identical across platforms.
pub fn quats_from_angles(angles: &[f32]) -> Vec<Quat> {
    angles.iter().map(|&a| quat_from_rotation_y(a)).collect()
}

/// `Quat::from_rotation_y` with a platform-deterministic trig path: a
/// Y-axis rotation of `angle` radians is `(0, sin(θ/2), 0, cos(θ/2))`,
/// with the half-angle trig taken from `libm` for bit-identical bytes
/// on every IEEE platform.
fn quat_from_rotation_y(angle: f32) -> Quat {
    let half = f64::from(angle) * 0.5;
    Quat::from_xyzw(0.0, libm::sin(half) as f32, 0.0, libm::cos(half) as f32)
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
// position relative to the hips) have real motion to measure. The rig
// itself is the shared `animsmith_core::fixtures` builder; here we bind
// the profile-resolving bone names and the clean/popped period counts.

use animsmith_core::fixtures::{WALK_STRIDE, WalkBones, walk_doc};

/// `pelvis` + `foot_l` / `foot_r`: bone names that resolve the built-in
/// `ue-mannequin` profile, so semantic checks fire under `profile =
/// "auto"` with no inline role map (unlike `semantic.rs`, which wires
/// explicit roles over `l_foot`/`r_foot`).
const WALK_BONES: WalkBones = WalkBones {
    hips: "pelvis",
    left_foot: "foot_l",
    right_foot: "foot_r",
};

/// The clean committed walk clip (`examples/assets/walk.glb`): a 1 s
/// cycle that closes exactly, so the loop seam is ≈ 0. `libm::sin` keeps
/// the committed bytes identical across platforms.
fn example_walk_doc() -> Document {
    walk_doc(&WALK_BONES, "walk", 1.0, WALK_STRIDE, libm::sin)
}

/// The popped-seam walk clip (`examples/assets/walk-dirty.glb`): the same
/// motion cut at ¾ of a cycle, so the feet never return to their
/// first-frame pose and the loop seam pops.
fn example_walk_dirty_doc() -> Document {
    walk_doc(&WALK_BONES, "walk", 0.75, WALK_STRIDE, libm::sin)
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
