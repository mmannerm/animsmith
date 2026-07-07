//! M3 checks: in-place, fps, bind-pose, foot-slide. The foot-slide
//! fixture is an analytic treadmill walk: flat stance at constant
//! sweep speed, sinusoidal swing — so the expected stance speed is
//! exact.

use animsmith_core::model::*;
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::{CheckCtx, Config, MetricGrids, Severity, all_checks, run_checks};
use glam::{Quat, Vec3};

const KEYS: usize = 33; // 32 intervals over 1 s
const STANCE_SWEEP_M: f32 = 0.5; // stance covers ±0.25 m in 0.5 s → 1 m/s

fn skeleton() -> Skeleton {
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
            Bone {
                name: "l_foot".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(0.1, -1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "r_foot".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(-0.1, -1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ],
    }
}

fn roles(skel: &Skeleton) -> ResolvedRoles {
    ResolvedRoles::from_names(
        skel,
        [
            (Role::Hips, "pelvis".to_string()),
            (Role::LeftFoot, "l_foot".to_string()),
            (Role::RightFoot, "r_foot".to_string()),
        ],
    )
}

/// Treadmill foot: stance (first half of the cycle, phase-offset per
/// foot) sweeps z linearly at constant speed with y = 0; swing lifts
/// the foot and returns it.
fn treadmill_track(bone: BoneId, rest: Vec3, phase_offset: f64, sweep: f32) -> Track {
    let times: Vec<f32> = (0..KEYS).map(|k| k as f32 / (KEYS - 1) as f32).collect();
    let values: Vec<Vec3> = (0..KEYS)
        .map(|k| {
            let u = ((k as f64 / (KEYS - 1) as f64) + phase_offset).rem_euclid(1.0);
            let (dy, dz) = if u < 0.5 {
                // Stance: z from +sweep/2 to −sweep/2, grounded.
                let s = u / 0.5;
                (0.0, (0.5 - s as f32) * sweep)
            } else {
                // Swing: return, lifted.
                let s = (u - 0.5) / 0.5;
                (
                    0.08 * (std::f64::consts::PI * s).sin() as f32,
                    (s as f32 - 0.5) * sweep,
                )
            };
            rest + Vec3::new(0.0, dy, dz)
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

fn treadmill_doc(sweep: f32) -> Document {
    let skel = skeleton();
    Document {
        skeleton: skel.clone(),
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![
                treadmill_track(1, skel.bones[1].rest.translation, 0.0, sweep),
                treadmill_track(2, skel.bones[2].rest.translation, 0.5, sweep),
            ],
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}

fn lint_with(doc: &Document, config: &Config) -> Vec<animsmith_core::Finding> {
    let roles = roles(&doc.skeleton);
    let grids = MetricGrids::new(doc);
    let ctx = CheckCtx::new(&grids, &roles, config);
    run_checks(&ctx, &all_checks())
}

fn json_config(json: serde_json::Value) -> Config {
    serde_json::from_value(json).expect("config parses")
}

fn of<'a>(findings: &'a [animsmith_core::Finding], id: &str) -> Vec<&'a animsmith_core::Finding> {
    findings.iter().filter(|f| f.check_id == id).collect()
}

// ---- foot-slide -------------------------------------------------------

#[test]
fn clean_treadmill_passes_foot_slide() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let findings = lint_with(&doc, &config);
    assert!(
        of(&findings, "foot-slide").is_empty(),
        "clean treadmill flagged: {findings:#?}"
    );
    // And the in-place treadmill exemption holds: no stray-pin error.
    assert!(of(&findings, "root-motion-speed").is_empty());
}

#[test]
fn slippery_stance_is_flagged() {
    // Stance sweeps at half the declared speed: 0.5 m/s deviation.
    let doc = treadmill_doc(STANCE_SWEEP_M * 0.5);
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let findings = lint_with(&doc, &config);
    let slides = of(&findings, "foot-slide");
    assert!(!slides.is_empty(), "slippery stance not flagged");
    assert_eq!(slides[0].severity, Severity::Warning);
}

/// #57: a rig whose feet resolve only as toe roles (no foot roles) must
/// still be judged — the per-foot loop falls back to the toe, matching
/// `foot_cycle_metrics`. Before the fix the loop skipped both feet and
/// produced silent nothing (readiness said Ready via root/hips).
#[test]
fn toe_only_rig_is_evaluated_for_foot_slide() {
    let doc = treadmill_doc(STANCE_SWEEP_M * 0.5); // slippery: 0.5 m/s deviation
    let roles = ResolvedRoles::from_names(
        &doc.skeleton,
        [
            (Role::Hips, "pelvis".to_string()),
            (Role::LeftToe, "l_foot".to_string()),
            (Role::RightToe, "r_foot".to_string()),
        ],
    );
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let findings = run_checks(&ctx, &all_checks());
    let slides = of(&findings, "foot-slide");
    assert!(
        !slides.is_empty(),
        "toe-only rig produced no foot-slide finding: {findings:#?}"
    );
    assert_eq!(slides[0].severity, Severity::Warning);
}

/// #100: when a side resolves *both* a foot and a toe role, foot-slide
/// must measure (and name) the foot — the `[foot, toe]` preference. The
/// foot bones are slippery (so they flag) while the toe bones are planted
/// cleanly; a foot-first loop names the foot, a toe-first regression
/// would measure the clean toe and either drop the finding or name the
/// toe. Locks the ordering the toe-only test (#57) can't see.
#[test]
fn foot_slide_prefers_foot_over_toe_when_both_resolve() {
    let bone = |name: &str, x: f32, z: f32| Bone {
        name: name.into(),
        parent: Some(0),
        rest: Transform {
            translation: Vec3::new(x, -1.0, z),
            ..Transform::IDENTITY
        },
        inverse_bind: None,
    };
    let skel = Skeleton {
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
            bone("l_foot", 0.1, 0.0),
            bone("r_foot", -0.1, 0.0),
            bone("l_toe", 0.1, 0.1),
            bone("r_toe", -0.1, 0.1),
        ],
    };
    let doc = Document {
        skeleton: skel.clone(),
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![
                // Slippery feet (half sweep → 0.5 m/s vs declared 1.0).
                treadmill_track(1, skel.bones[1].rest.translation, 0.0, STANCE_SWEEP_M * 0.5),
                treadmill_track(2, skel.bones[2].rest.translation, 0.5, STANCE_SWEEP_M * 0.5),
                // Clean toes (full sweep → exactly the declared 1.0 m/s).
                treadmill_track(3, skel.bones[3].rest.translation, 0.0, STANCE_SWEEP_M),
                treadmill_track(4, skel.bones[4].rest.translation, 0.5, STANCE_SWEEP_M),
            ],
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    };
    let roles = ResolvedRoles::from_names(
        &skel,
        [
            (Role::Hips, "pelvis".to_string()),
            (Role::LeftFoot, "l_foot".to_string()),
            (Role::RightFoot, "r_foot".to_string()),
            (Role::LeftToe, "l_toe".to_string()),
            (Role::RightToe, "r_toe".to_string()),
        ],
    );
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let findings = run_checks(&ctx, &all_checks());
    let slides = of(&findings, "foot-slide");
    assert!(
        !slides.is_empty(),
        "slippery foot must flag (toe-first regression drops it): {findings:#?}"
    );
    for f in &slides {
        let named = f.bone.as_deref().expect("finding names a bone");
        assert!(
            named == "l_foot" || named == "r_foot",
            "foot-slide measured `{named}`, expected the foot bone — toe-first regression?"
        );
    }
}

// ---- in-place ---------------------------------------------------------

#[test]
fn travelling_clip_declared_in_place_is_flagged() {
    let mut doc = treadmill_doc(STANCE_SWEEP_M);
    doc.clips[0].tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 2.0)]),
    });
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "in_place": true } }
    }));
    let findings = lint_with(&doc, &config);
    let hits = of(&findings, "in-place");
    assert_eq!(hits.len(), 1, "got: {findings:#?}");
    assert_eq!(hits[0].severity, Severity::Error);
}

#[test]
fn stationary_clip_declared_root_motion_is_flagged() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "in_place": false } }
    }));
    let findings = lint_with(&doc, &config);
    assert_eq!(of(&findings, "in-place").len(), 1);
}

#[test]
fn matching_in_place_declaration_is_clean() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "in_place": true } }
    }));
    let findings = lint_with(&doc, &config);
    assert!(of(&findings, "in-place").is_empty(), "got: {findings:#?}");
}

// ---- fps --------------------------------------------------------------

#[test]
fn on_grid_keys_pass_fps() {
    let doc = treadmill_doc(STANCE_SWEEP_M); // keys at k/32 over 1 s
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "fps": 32.0 } }
    }));
    let findings = lint_with(&doc, &config);
    assert!(of(&findings, "fps").is_empty(), "got: {findings:#?}");
}

#[test]
fn off_grid_key_and_fractional_duration_are_flagged() {
    let mut doc = treadmill_doc(STANCE_SWEEP_M);
    doc.clips[0].tracks[0].times[5] += 0.011; // ~0.35 frames off at 32 fps
    doc.clips[0].duration_s = 1.013; // 32.4 frames
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "fps": 32.0 } }
    }));
    let findings = lint_with(&doc, &config);
    assert_eq!(of(&findings, "fps").len(), 2, "got: {findings:#?}");
}

// ---- bind-pose --------------------------------------------------------

fn rotated_first_frame_doc(angle: f32) -> Document {
    let skel = skeleton();
    let tracks = (0..3)
        .map(|bone| Track {
            bone,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![
                Quat::from_rotation_y(angle),
                Quat::from_rotation_y(angle + 0.1),
            ]),
        })
        .collect();
    Document {
        skeleton: skel,
        clips: vec![Clip {
            name: "pose".into(),
            duration_s: 1.0,
            tracks,
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}

#[test]
fn wrong_bind_is_flagged() {
    // Every bone starts 90° from rest: not a plausible start pose.
    let findings = lint_with(
        &rotated_first_frame_doc(std::f32::consts::FRAC_PI_2),
        &Config::default(),
    );
    let hits = of(&findings, "bind-pose");
    assert_eq!(hits.len(), 1, "got: {findings:#?}");
}

#[test]
fn near_rest_start_is_clean() {
    let findings = lint_with(&rotated_first_frame_doc(0.15), &Config::default());
    assert!(of(&findings, "bind-pose").is_empty(), "got: {findings:#?}");
}
