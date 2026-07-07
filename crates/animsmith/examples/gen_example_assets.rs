//! Regenerates the committed example assets under `examples/assets/`:
//! a tiny clean two-bone rotation clip (`clip.glb`) and a byte-for-byte
//! copy with two deliberate, repairable defects injected —
//! `clip-dirty.glb`. The dirty copy carries one non-unit rotation key
//! (`quat-norm`) and one sign-flipped key (`quat-flip`); everything else
//! matches the clean clip, so `fix` restores it exactly and `diff`
//! shows no measurement drift.
//!
//! The assets are procedurally generated rather than hand-authored so
//! their provenance is this file and they stay regenerable. The dirty
//! copy is `.glb` on purpose: `fix` is byte-surgical over a GLB binary
//! chunk and skips the data-URI buffers a `.gltf` would embed.
//!
//! Run (writes to the repo's `examples/assets/`):
//!   cargo run -p animsmith --example gen_example_assets
//! Write elsewhere:
//!   cargo run -p animsmith --example gen_example_assets -- /some/dir

use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::*;
use std::path::{Path, PathBuf};

/// Five unit Y-rotation keys, all in the same hemisphere (small angles,
/// positive `w`), stepping through a gentle swing.
fn clean_quats() -> Vec<Quat> {
    [0.0f32, 0.1, 0.2, 0.3, 0.4]
        .iter()
        .map(|&a| Quat::from_rotation_y(a))
        .collect()
}

fn scaled(q: Quat, scale: f32) -> Quat {
    let [x, y, z, w] = q.to_array();
    Quat::from_xyzw(x * scale, y * scale, z * scale, w * scale)
}

/// A two-bone rig (`root` -> `spine`) with one rotation clip. The clip
/// name is `swing`; keys sit on a clean 0.0/0.25/0.5/0.75/1.0 s grid.
fn doc_with_quats(quats: Vec<Quat>) -> Document {
    Document {
        skeleton: Skeleton {
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
        },
        clips: vec![Clip {
            name: "swing".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 0.25, 0.5, 0.75, 1.0],
                values: TrackValues::Quats(quats),
            }],
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}

/// The clean clip with two defects injected: key 2 scaled off the unit
/// sphere (`quat-norm`) and key 3 negated into the opposite hemisphere
/// (`quat-flip`). Both are lossless to repair — scaling and sign do not
/// change the represented rotation.
fn dirty_quats() -> Vec<Quat> {
    let mut q = clean_quats();
    q[2] = scaled(q[2], 1.05);
    q[3] = -q[3];
    q
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/assets"));
    std::fs::create_dir_all(&out_dir)?;

    let clean_path = out_dir.join("clip.glb");
    animsmith_gltf::write::write(&doc_with_quats(clean_quats()), &clean_path)?;
    println!("wrote {} (clean)", clean_path.display());

    let dirty_path = out_dir.join("clip-dirty.glb");
    animsmith_gltf::write::write(&doc_with_quats(dirty_quats()), &dirty_path)?;
    println!(
        "wrote {} (quat-norm + quat-flip defects)",
        dirty_path.display()
    );

    Ok(())
}
