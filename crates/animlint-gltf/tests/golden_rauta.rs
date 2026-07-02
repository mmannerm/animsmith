//! Golden test against the rauta project's production bake: the ported
//! loop-seam and gait algorithms must reproduce the numbers rauta's
//! Python reference (`locomotion_metrics.py`) recorded in
//! `character.measured.ron` (which were themselves verified against
//! Blender pose-matrix FK to <0.01×).
//!
//! Gated on RAUTA_CHARACTER_GLB because the asset is licensed and can't
//! ship in this repo:
//!
//! ```console
//! RAUTA_CHARACTER_GLB=~/src/rauta/assets/models/character.glb cargo test -p animlint-gltf golden
//! ```

use animlint_core::detect_profile;
use animlint_core::measure::measure_document;

/// (clip, loop_seam_ratio, gait phase, lr_amplitude_m) as recorded by
/// rauta's Python reference. Seam ratios use `None` where the reference
/// reports no real stride.
const GOLDEN: &[(&str, Option<f64>, f64, f64)] = &[
    ("idle_1h", Some(0.997817), 0.658668, 0.016100),
    ("run_forward_1h", Some(0.806956), 0.030621, 0.097489),
    ("run_backward_1h", Some(0.794718), 0.005160, 0.091987),
    ("run_left_1h", Some(0.665597), 0.086954, 0.155317),
    ("run_right_1h", Some(0.933476), 0.035452, 0.102803),
    ("run_forward_left_1h", Some(0.547616), 0.949657, 0.100370),
    ("walk_forward_1h", Some(0.638331), 0.018975, 0.160022),
    ("walk_backward_right_1h", Some(0.914495), 0.005203, 0.154465),
    ("block_1h", None, 0.486718, 0.000000),
    ("jump_1h", Some(3.957754), 0.144650, 0.025147),
];

const SEAM_TOLERANCE: f64 = 0.02;
const PHASE_TOLERANCE: f64 = 0.02;
const AMPLITUDE_TOLERANCE: f64 = 0.005;

fn circular_delta(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(1.0);
    d.min(1.0 - d)
}

#[test]
fn reproduces_rauta_reference_numbers() {
    let Ok(path) = std::env::var("RAUTA_CHARACTER_GLB") else {
        eprintln!("skipped: set RAUTA_CHARACTER_GLB to run the golden test");
        return;
    };
    let doc = animlint_gltf::load(std::path::Path::new(&path)).expect("golden GLB loads");
    let roles = detect_profile(&doc.skeleton).expect("rauta profile detected");
    assert_eq!(roles.profile, "rauta-humanoid");
    let measurements = measure_document(&doc, &roles);

    let mut failures = Vec::new();
    for &(clip, want_seam, want_phase, want_amplitude) in GOLDEN {
        let Some(m) = measurements.get(clip) else {
            failures.push(format!("{clip}: missing from measurements"));
            continue;
        };
        match (want_seam, m.loop_seam_ratio) {
            (Some(want), Some(got)) if (want - got).abs() > SEAM_TOLERANCE => {
                failures.push(format!("{clip}: seam {got:.6} != golden {want:.6}"));
            }
            (Some(want), None) => {
                failures.push(format!("{clip}: seam missing, golden {want:.6}"));
            }
            (None, Some(got)) => {
                failures.push(format!("{clip}: seam {got:.6}, golden reports none"));
            }
            _ => {}
        }
        let gait = m.gait.as_ref().expect("gait present");
        let phase = gait.phase.expect("phase present");
        // The phase of a (near-)zero-amplitude signal is numerical
        // noise — the same reason the gait-group check carries a
        // min_lr_amplitude_m confidence floor. Compare it only where
        // the reference amplitude is meaningful.
        if want_amplitude > 0.005 && circular_delta(phase, want_phase) > PHASE_TOLERANCE {
            failures.push(format!(
                "{clip}: phase {phase:.6} != golden {want_phase:.6}"
            ));
        }
        if (gait.lr_amplitude_m - want_amplitude).abs() > AMPLITUDE_TOLERANCE {
            failures.push(format!(
                "{clip}: amplitude {:.6} != golden {want_amplitude:.6}",
                gait.lr_amplitude_m
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "golden mismatches:\n{}",
        failures.join("\n")
    );
}
