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
/// via `quat_from_rotation_y`, whose `libm` trig keeps the committed
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

// --- Raw glTF scale fixtures ------------------------------------------------

/// The compensated skinned hierarchy of DESIGN.md Appendix D §D.3 case 2, as
/// self-contained GLB bytes.
///
/// This is the one shape both accepted scale operations run on end to end, so
/// it is built here rather than inside any single test: a CLI test lives in a
/// different crate from the glTF frontend's own hostile-variant builders and
/// must not re-derive the rig from memory.
///
/// # The rig
///
/// ```text
/// node 0 "root"    scale (0.01, 0.01, 0.01)   the scaled ancestor
/// node 1 "joint"   translation (0, 100, 0)    the skin's only joint
/// node 2 "attach"  translation (1, 0, 0)      a transform-only child
/// node 3 "holder"  mesh 0, skin 0             outside the closure
/// ```
///
/// The skin's single inverse bind is `diag(100)` with translation
/// `(0, -100, 0)` — §D.1's "inverse-bind matrices carry a compensating linear
/// magnitude of 100" — so `W(rest) * B = I` and the mesh's vertices are
/// already authored in the correct world space. One animation carries a
/// `LINEAR` translation track and a `LINEAR` rotation track on the joint.
///
/// Every literal here is authored, not computed: a fixture whose values are
/// derived by the same arithmetic a test is checking cannot falsify it.
pub fn rest_bind_scale_rig_glb() -> Vec<u8> {
    glb_container(
        rest_bind_scale_rig_json("").as_bytes(),
        &rest_bind_scale_rig_buffer(),
    )
}

/// The same rig as [`rest_bind_scale_rig_glb`], as a self-contained JSON
/// `.gltf` whose single buffer is a base64 data URI.
///
/// The two containers carry byte-identical payloads, so a producer that
/// treats one correctly and the other not is visible as a difference in the
/// artifact rather than in the fixture.
pub fn rest_bind_scale_rig_gltf() -> Vec<u8> {
    use base64::Engine as _;

    let buffer = rest_bind_scale_rig_buffer();
    let uri = format!(
        "\"uri\": \"data:application/octet-stream;base64,{}\", ",
        base64::engine::general_purpose::STANDARD.encode(&buffer)
    );
    rest_bind_scale_rig_json(&uri).into_bytes()
}

/// The rig's 412-byte payload, shared by both containers.
fn rest_bind_scale_rig_buffer() -> Vec<u8> {
    const LENGTH: usize = 412;
    const AT_POSITION: usize = 0;
    const AT_WEIGHTS: usize = 60;
    const AT_INVERSE_BIND: usize = 108;
    const AT_TIMES: usize = 172;
    const AT_TRANSLATION: usize = 180;
    const AT_ROTATION: usize = 252;

    let mut buffer = vec![0u8; LENGTH];
    let mut put_f32 = |offset: usize, values: &[f32]| {
        for (index, value) in values.iter().enumerate() {
            buffer[offset + index * 4..offset + index * 4 + 4]
                .copy_from_slice(&value.to_le_bytes());
        }
    };
    put_f32(
        AT_POSITION,
        &[0.0, 1.0, 0.0, 0.5, 1.25, -0.25, -0.5, 0.75, 0.5],
    );
    // JOINTS_0 occupies 36..60 as twelve `u16` zeros, which the zero-filled
    // buffer already is.
    put_f32(
        AT_WEIGHTS,
        &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
    );
    put_f32(
        AT_INVERSE_BIND,
        &[
            100.0, 0.0, 0.0, 0.0, //
            0.0, 100.0, 0.0, 0.0, //
            0.0, 0.0, 100.0, 0.0, //
            0.0, -100.0, 0.0, 1.0,
        ],
    );
    put_f32(AT_TIMES, &[0.0, 1.0]);
    put_f32(AT_TRANSLATION, &[0.0, 100.0, 0.0, 0.0, 300.0, 0.0]);
    put_f32(AT_ROTATION, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    buffer
}

/// The rig's JSON, with `buffer_uri` spliced into its single `buffers` entry:
/// empty for a GLB, whose payload is the BIN chunk, and a data URI for a
/// self-contained `.gltf`.
fn rest_bind_scale_rig_json(buffer_uri: &str) -> String {
    format!(
        r#"{{
  "asset": {{ "version": "2.0", "generator": "animsmith test rig" }},
  "buffers": [{{ {buffer_uri}"byteLength": 412 }}],
  "bufferViews": [
    {{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }},
    {{ "buffer": 0, "byteOffset": 36, "byteLength": 24 }},
    {{ "buffer": 0, "byteOffset": 60, "byteLength": 48 }},
    {{ "buffer": 0, "byteOffset": 108, "byteLength": 64 }},
    {{ "buffer": 0, "byteOffset": 172, "byteLength": 8 }},
    {{ "buffer": 0, "byteOffset": 180, "byteLength": 24 }},
    {{ "buffer": 0, "byteOffset": 252, "byteLength": 32 }}
  ],
  "accessors": [
    {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [-0.5, 0.75, -0.25], "max": [0.5, 1.25, 0.5] }},
    {{ "bufferView": 1, "componentType": 5123, "count": 3, "type": "VEC4" }},
    {{ "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4" }},
    {{ "bufferView": 3, "componentType": 5126, "count": 1, "type": "MAT4" }},
    {{ "bufferView": 4, "componentType": 5126, "count": 2, "type": "SCALAR",
      "min": [0.0], "max": [1.0] }},
    {{ "bufferView": 5, "componentType": 5126, "count": 2, "type": "VEC3" }},
    {{ "bufferView": 6, "componentType": 5126, "count": 2, "type": "VEC4" }}
  ],
  "materials": [{{ "name": "surface", "alphaCutoff": 0.25 }}],
  "meshes": [{{ "primitives": [{{
    "attributes": {{ "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2 }},
    "material": 0
  }}] }}],
  "nodes": [
    {{ "name": "root", "scale": [0.01, 0.01, 0.01], "children": [1] }},
    {{ "name": "joint", "translation": [0.0, 100.0, 0.0], "children": [2] }},
    {{ "name": "attach", "translation": [1.0, 0.0, 0.0] }},
    {{ "name": "holder", "mesh": 0, "skin": 0 }}
  ],
  "scenes": [{{ "nodes": [0, 3] }}],
  "scene": 0,
  "skins": [{{ "joints": [1], "skeleton": 0, "inverseBindMatrices": 3 }}],
  "animations": [{{
    "name": "clip",
    "samplers": [
      {{ "input": 4, "interpolation": "LINEAR", "output": 5 }},
      {{ "input": 4, "interpolation": "LINEAR", "output": 6 }}
    ],
    "channels": [
      {{ "sampler": 0, "target": {{ "node": 1, "path": "translation" }} }},
      {{ "sampler": 1, "target": {{ "node": 1, "path": "rotation" }} }}
    ]
  }}]
}}"#
    )
}

/// The §D.3 case 2 rig with a **second, disjoint** skinned instance that the
/// rest/bind operation on skin 0 does not touch, as self-contained GLB bytes.
///
/// ```text
/// node 4 "joint2"   no transform      skin 1's only joint, outside the closure
/// node 5 "holder2"  mesh 0, skin 1    a second instance of the same mesh
/// ```
///
/// Skin 1 carries **its own** identity inverse-bind accessor rather than
/// sharing skin 0's: two skins pointing at one `inverseBindMatrices` accessor
/// are refused with `conflicting-rest-bind-factor`, because one accessor
/// cannot be both rewritten by the factor and left alone.
///
/// This is the only shape in which the `unaffected_inverse_bind` obligation
/// has something to walk. On [`rest_bind_scale_rig_glb`] the single skinned
/// instance's joint is *inside* the affected closure, so that residual reports
/// an absence; here the run publishes a measured `0.0` — measured because the
/// rewrite must leave node 4's world rest and skin 1's bind exactly as it
/// found them.
///
/// The rig's buffer already declares 412 bytes while its views stop at 284, so
/// the second inverse bind occupies that declared-but-unused tail and the
/// payload length is unchanged.
pub fn unaffected_bind_scale_rig_glb() -> Vec<u8> {
    glb_container(
        UNAFFECTED_BIND_SCALE_RIG_JSON.as_bytes(),
        &unaffected_bind_scale_rig_buffer(),
    )
}

/// [`rest_bind_scale_rig_buffer`] with skin 1's identity inverse bind written
/// into the payload's unused tail at byte 284.
fn unaffected_bind_scale_rig_buffer() -> Vec<u8> {
    const AT_SECOND_INVERSE_BIND: usize = 284;

    let mut buffer = rest_bind_scale_rig_buffer();
    let identity: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ];
    for (index, value) in identity.iter().enumerate() {
        let at = AT_SECOND_INVERSE_BIND + index * 4;
        buffer[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    buffer
}

/// The JSON of [`unaffected_bind_scale_rig_glb`].
///
/// Written out in full rather than spliced into
/// [`rest_bind_scale_rig_json`]: the two fixtures differ in five places —
/// buffer view, accessor, two nodes, the scene, and the skins array — and a
/// builder carrying five conditional holes would make neither of them
/// readable.
const UNAFFECTED_BIND_SCALE_RIG_JSON: &str = r#"{
  "asset": { "version": "2.0", "generator": "animsmith test rig" },
  "buffers": [{ "byteLength": 412 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
    { "buffer": 0, "byteOffset": 36, "byteLength": 24 },
    { "buffer": 0, "byteOffset": 60, "byteLength": 48 },
    { "buffer": 0, "byteOffset": 108, "byteLength": 64 },
    { "buffer": 0, "byteOffset": 172, "byteLength": 8 },
    { "buffer": 0, "byteOffset": 180, "byteLength": 24 },
    { "buffer": 0, "byteOffset": 252, "byteLength": 32 },
    { "buffer": 0, "byteOffset": 284, "byteLength": 64 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [-0.5, 0.75, -0.25], "max": [0.5, 1.25, 0.5] },
    { "bufferView": 1, "componentType": 5123, "count": 3, "type": "VEC4" },
    { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4" },
    { "bufferView": 3, "componentType": 5126, "count": 1, "type": "MAT4" },
    { "bufferView": 4, "componentType": 5126, "count": 2, "type": "SCALAR",
      "min": [0.0], "max": [1.0] },
    { "bufferView": 5, "componentType": 5126, "count": 2, "type": "VEC3" },
    { "bufferView": 6, "componentType": 5126, "count": 2, "type": "VEC4" },
    { "bufferView": 7, "componentType": 5126, "count": 1, "type": "MAT4" }
  ],
  "materials": [{ "name": "surface", "alphaCutoff": 0.25 }],
  "meshes": [{ "primitives": [{
    "attributes": { "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2 },
    "material": 0
  }] }],
  "nodes": [
    { "name": "root", "scale": [0.01, 0.01, 0.01], "children": [1] },
    { "name": "joint", "translation": [0.0, 100.0, 0.0], "children": [2] },
    { "name": "attach", "translation": [1.0, 0.0, 0.0] },
    { "name": "holder", "mesh": 0, "skin": 0 },
    { "name": "joint2" },
    { "name": "holder2", "mesh": 0, "skin": 1 }
  ],
  "scenes": [{ "nodes": [0, 3, 4, 5] }],
  "scene": 0,
  "skins": [
    { "joints": [1], "skeleton": 0, "inverseBindMatrices": 3 },
    { "joints": [4], "skeleton": 4, "inverseBindMatrices": 7 }
  ],
  "animations": [{
    "name": "clip",
    "samplers": [
      { "input": 4, "interpolation": "LINEAR", "output": 5 },
      { "input": 4, "interpolation": "LINEAR", "output": 6 }
    ],
    "channels": [
      { "sampler": 0, "target": { "node": 1, "path": "translation" } },
      { "sampler": 1, "target": { "node": 1, "path": "rotation" } }
    ]
  }]
}"#;

/// The §D.3 case 2 hierarchy with **no meshes and no animations**, as
/// self-contained GLB bytes.
///
/// ```text
/// node 0 "root"    scale (0.01, 0.01, 0.01)   the scaled ancestor
/// node 1 "joint"   translation (0, 100, 0)    the skin's only joint
/// node 2 "attach"  translation (1, 0, 0)      a transform-only child
/// ```
///
/// Skin 0 carries the same `diag(100)` inverse bind as
/// [`rest_bind_scale_rig_glb`] but is referenced by no node, so the document
/// has no mesh instance and no clip.
///
/// It exists because three of the twelve published residual predicates —
/// "the source has a clip track value", "the source has a base mesh
/// `POSITION`", and the rest obligation — are `true` on every other fixture,
/// and a predicate whose `false` branch is never observed is a predicate a
/// constant `true` would satisfy. On this rig the first two report absences
/// while the rest obligation stays evaluated, which separates all three.
pub fn nodes_only_scale_rig_glb() -> Vec<u8> {
    glb_container(
        NODES_ONLY_SCALE_RIG_JSON.as_bytes(),
        &nodes_only_scale_rig_buffer(),
    )
}

/// The nodes-only rig's 64-byte payload: skin 0's inverse bind, `diag(100)`
/// with translation `(0, -100, 0)`.
fn nodes_only_scale_rig_buffer() -> Vec<u8> {
    let inverse_bind: [f32; 16] = [
        100.0, 0.0, 0.0, 0.0, //
        0.0, 100.0, 0.0, 0.0, //
        0.0, 0.0, 100.0, 0.0, //
        0.0, -100.0, 0.0, 1.0,
    ];
    inverse_bind
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// The JSON of [`nodes_only_scale_rig_glb`].
const NODES_ONLY_SCALE_RIG_JSON: &str = r#"{
  "asset": { "version": "2.0", "generator": "animsmith test rig" },
  "buffers": [{ "byteLength": 64 }],
  "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 64 }],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 1, "type": "MAT4" }
  ],
  "nodes": [
    { "name": "root", "scale": [0.01, 0.01, 0.01], "children": [1] },
    { "name": "joint", "translation": [0.0, 100.0, 0.0], "children": [2] },
    { "name": "attach", "translation": [1.0, 0.0, 0.0] }
  ],
  "scenes": [{ "nodes": [0] }],
  "scene": 0,
  "skins": [{ "joints": [1], "skeleton": 0, "inverseBindMatrices": 0 }]
}"#;

/// How many vertices [`oversized_proof_scale_rig_glb`]'s single primitive
/// carries.
const OVERSIZED_VERTEX_COUNT: usize = 50_000;

/// How many key times [`oversized_proof_scale_rig_glb`]'s single translation
/// track carries.
const OVERSIZED_KEY_COUNT: usize = 5_000;

/// A well-formed skinned rig whose *size alone* puts the sampled proof over
/// `ScaleTolerancePolicy::proof_sample_work_budget`, as self-contained GLB
/// bytes (about 1.9 MB).
///
/// ```text
/// node 0 "root"    no transform
/// node 1 "joint"   translation (0, 100, 0)    skin 0's only joint
/// node 2 "holder"  mesh 0, skin 0             50,000 vertices, one influence
/// ```
///
/// Hand-computed against the `appendix-d-v5` budget of `400_000_000`, using
/// the per-sample charge the core proof documents — two document sides times
/// the bone count, plus two sides and one residual comparison per skin slot,
/// plus two sides times the skinned vertex count:
///
/// ```text
/// per-sample cost = 2 sides * 3 bones                    =        6
///                 + (2 sides + 1 skin residual) * 1 slot =        3
///                 + 2 sides * 50,000 skinned vertices    =  100,000
///                                                          ---------
///                                                            100,009
/// sample times    = 5,000 LINEAR keys, no cubic segments
/// work            = 5,000 * 100,009 = 500,045,000
/// budget          =                   400,000,000
/// ```
///
/// It exists because it is the only way to reach a **proof-stage** refusal
/// from the CLI: every other proof rejection needs a residual the rewrite
/// cannot make small, which a correct rewriter does not produce. The refusal
/// is raised before the first sample time is evaluated, so this fixture is
/// cheap to run despite its size.
///
/// Every payload value is a constant rather than a ramp: the fixture's job is
/// its dimensions, and a varying payload would put arithmetic the proof never
/// reaches into a test about refusing to start.
pub fn oversized_proof_scale_rig_glb() -> Vec<u8> {
    glb_container(
        oversized_proof_scale_rig_json().as_bytes(),
        &oversized_proof_scale_rig_buffer(),
    )
}

/// The oversized rig's payload, in the order its buffer views declare:
/// positions, joint indices, weights, the inverse bind, key times, and the
/// translation track's values.
fn oversized_proof_scale_rig_buffer() -> Vec<u8> {
    let mut buffer = Vec::with_capacity(1_880_064);
    let push_f32 = |buffer: &mut Vec<u8>, values: &[f32]| {
        for value in values {
            buffer.extend_from_slice(&value.to_le_bytes());
        }
    };
    for _ in 0..OVERSIZED_VERTEX_COUNT {
        push_f32(&mut buffer, &[0.0, 1.0, 0.0]);
    }
    // JOINTS_0: four `u16` zeros per vertex, every vertex bound to joint 0.
    buffer.extend(std::iter::repeat_n(0u8, OVERSIZED_VERTEX_COUNT * 8));
    for _ in 0..OVERSIZED_VERTEX_COUNT {
        push_f32(&mut buffer, &[1.0, 0.0, 0.0, 0.0]);
    }
    push_f32(
        &mut buffer,
        &[
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ],
    );
    // Strictly increasing, and every one of them exactly representable in
    // `f32`, so the key times are 5,000 distinct sample times and not fewer.
    for key in 0..OVERSIZED_KEY_COUNT {
        push_f32(&mut buffer, &[key as f32 * 0.5]);
    }
    for _ in 0..OVERSIZED_KEY_COUNT {
        push_f32(&mut buffer, &[0.0, 100.0, 0.0]);
    }
    buffer
}

/// The JSON of [`oversized_proof_scale_rig_glb`]. The counts and byte offsets
/// are spliced from the two dimension constants so the JSON and the payload
/// cannot disagree.
fn oversized_proof_scale_rig_json() -> String {
    let positions = OVERSIZED_VERTEX_COUNT * 12;
    let joints = OVERSIZED_VERTEX_COUNT * 8;
    let weights = OVERSIZED_VERTEX_COUNT * 16;
    let times = OVERSIZED_KEY_COUNT * 4;
    let translations = OVERSIZED_KEY_COUNT * 12;
    let at_joints = positions;
    let at_weights = at_joints + joints;
    let at_inverse_bind = at_weights + weights;
    let at_times = at_inverse_bind + 64;
    let at_translations = at_times + times;
    let vertices = OVERSIZED_VERTEX_COUNT;
    let keys = OVERSIZED_KEY_COUNT;
    let last_time = (keys - 1) as f32 * 0.5;
    format!(
        r#"{{
  "asset": {{ "version": "2.0", "generator": "animsmith budget rig" }},
  "buffers": [{{ "byteLength": {total} }}],
  "bufferViews": [
    {{ "buffer": 0, "byteOffset": 0, "byteLength": {positions} }},
    {{ "buffer": 0, "byteOffset": {at_joints}, "byteLength": {joints} }},
    {{ "buffer": 0, "byteOffset": {at_weights}, "byteLength": {weights} }},
    {{ "buffer": 0, "byteOffset": {at_inverse_bind}, "byteLength": 64 }},
    {{ "buffer": 0, "byteOffset": {at_times}, "byteLength": {times} }},
    {{ "buffer": 0, "byteOffset": {at_translations}, "byteLength": {translations} }}
  ],
  "accessors": [
    {{ "bufferView": 0, "componentType": 5126, "count": {vertices}, "type": "VEC3",
      "min": [0.0, 1.0, 0.0], "max": [0.0, 1.0, 0.0] }},
    {{ "bufferView": 1, "componentType": 5123, "count": {vertices}, "type": "VEC4" }},
    {{ "bufferView": 2, "componentType": 5126, "count": {vertices}, "type": "VEC4" }},
    {{ "bufferView": 3, "componentType": 5126, "count": 1, "type": "MAT4" }},
    {{ "bufferView": 4, "componentType": 5126, "count": {keys}, "type": "SCALAR",
      "min": [0.0], "max": [{last_time:?}] }},
    {{ "bufferView": 5, "componentType": 5126, "count": {keys}, "type": "VEC3" }}
  ],
  "meshes": [{{ "primitives": [{{
    "attributes": {{ "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2 }}
  }}] }}],
  "nodes": [
    {{ "name": "root", "children": [1] }},
    {{ "name": "joint", "translation": [0.0, 100.0, 0.0] }},
    {{ "name": "holder", "mesh": 0, "skin": 0 }}
  ],
  "scenes": [{{ "nodes": [0, 2] }}],
  "scene": 0,
  "skins": [{{ "joints": [1], "skeleton": 0, "inverseBindMatrices": 3 }}],
  "animations": [{{
    "name": "clip",
    "samplers": [{{ "input": 4, "interpolation": "LINEAR", "output": 5 }}],
    "channels": [
      {{ "sampler": 0, "target": {{ "node": 1, "path": "translation" }} }}
    ]
  }}]
}}"#,
        total = at_translations + translations,
    )
}

/// Wrap a JSON chunk and a BIN chunk in a glTF 2.0 GLB container, padding
/// each chunk to the four-byte alignment the specification requires.
fn glb_container(json: &[u8], bin: &[u8]) -> Vec<u8> {
    const JSON_CHUNK: u32 = 0x4e4f_534a;
    const BIN_CHUNK: u32 = 0x004e_4942;

    let mut json = json.to_vec();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut bin = bin.to_vec();
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&JSON_CHUNK.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&BIN_CHUNK.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}
