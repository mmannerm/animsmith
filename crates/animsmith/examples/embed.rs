//! The compiling companion to docs/embedding.md: load a file, resolve
//! rig roles, declare expectations programmatically, measure, lint,
//! and map severities to a gate — the five steps an embedding pipeline
//! performs through the library API instead of the CLI.
//!
//! Run: `cargo run -p animsmith --example embed`

use animsmith_core::config::{ClipExpectations, Pinned};
use animsmith_core::measure::measure_document;
use animsmith_core::profile::{ResolvedRoles, Role, detect_profile};
use animsmith_core::{
    CheckCtx, CheckSelection, Config, LintEnvelope, LintFileReport, MeasurementContract,
    MetricGrids, RigInfo, Severity, ToolInfo, ToolSource, all_checks, evaluate_checks,
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load: values arrive exactly as authored.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/rig.gltf");
    let doc = animsmith_gltf::load(&path)?;
    println!(
        "loaded {} bones, {} clip(s)",
        doc.skeleton.bones.len(),
        doc.clips.len()
    );

    // 2. Resolve rig roles: auto-detect a built-in profile, or — as
    //    here, where the toy fixture matches none — bind roles
    //    explicitly. Checks whose roles don't resolve return a typed,
    //    nonblocking coverage gap; they never invent a content finding.
    let roles = detect_profile(&doc.skeleton).unwrap_or_else(|| {
        ResolvedRoles::from_names(
            &doc.skeleton,
            [
                (Role::Root, "root".to_string()),
                (Role::Hips, "hips".to_string()),
                (Role::LeftFoot, "foot".to_string()),
            ],
        )
    });
    println!("rig profile: {} ({} roles)", roles.profile, roles.len());

    // 3. Declare expectations programmatically. A pipeline builds
    //    this from its own contract format — the TOML file the CLI
    //    reads is just one constructor of the same struct.
    let mut config = Config::default();
    config.clips.insert(
        "walk".into(),
        ClipExpectations {
            looping: Some(true),
            // Deliberately wrong: the fixture's root travels 1 m/s, so
            // this declaration produces an `in-place` Error below —
            // demonstrating a finding and the non-zero gate exit.
            in_place: Some(true),
            fps: Some(2.0), // the fixture keys at 0.0/0.5/1.0 s
            speed_mps: None,
            animates_bones: Some(vec!["hips".into()]),
        },
    );

    // 4a. Measure: the raw metric map, no judgment. Share the metric
    //     grids with linting so each clip is sampled once.
    let grids = MetricGrids::new(&doc);
    let measurements = measure_document(&grids, &roles, &config);
    for (clip, m) in &measurements {
        println!(
            "measured '{clip}': {:.3}s, {} frames, {} animated bones",
            m.duration_s,
            m.frame_count,
            m.animated_bones.len()
        );
    }

    // 4b. Lint: the same sampled grids judged against the declarations.
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let evaluations = evaluate_checks(&ctx, &all_checks(), CheckSelection::All)?;
    let findings: Vec<_> = evaluations
        .iter()
        .flat_map(|check| check.findings())
        .collect();
    for f in &findings {
        println!(
            "  {}[{}] {}: {}",
            f.severity,
            f.check_id,
            f.clip.as_deref().unwrap_or("-"),
            f.message
        );
    }

    // 5. Map severities to your gate (the CLI's exit-code convention).
    let worst = findings.iter().map(|f| f.severity).max();
    let exit = match worst {
        Some(Severity::Error) => 1,
        _ => 0,
    };
    println!(
        "{} finding(s); gate exit code {exit} (expected 1 — see the \
         deliberately wrong in_place declaration above)",
        findings.len()
    );

    // When the host interoperates with CLI consumers, it can emit the exact
    // same versioned envelope without copying wire structs or schema URNs.
    let report = LintEnvelope::new(
        ToolInfo::animsmith(env!("CARGO_PKG_VERSION"), ToolSource::new(None, None)),
        vec![LintFileReport::new(
            path.display().to_string(),
            RigInfo::from_resolved(&doc, &roles),
            evaluations,
            MeasurementContract::new(
                measurements,
                animsmith_core::measure::measure_meshes(&doc.assets),
            ),
        )],
    );
    println!("result contract: {}", serde_json::to_string(&report)?);
    std::process::exit(exit);
}

// Ensure a Pinned expectation compiles the way the docs show it, even
// though the toy fixture declares no speed.
#[allow(dead_code)]
fn speed_pin() -> Pinned {
    Pinned {
        value: 3.1,
        tolerance: 0.25,
    }
}
