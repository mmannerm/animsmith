//! Drift guard for the committed documentation visuals under
//! `docs/visuals/`.
//!
//! The generator (`crates/animsmith/examples/gen_docs_visuals.rs`) and
//! this test drive the same `animsmith-testkit` wiring, so a renamed
//! output, a dropped chart, a changed report argument, or a change in
//! the report renderer itself fails here rather than when a human next
//! remembers to rerun the generator. This test runs the built CLI
//! directly; the generator reaches the same binary through Cargo.
//!
//! `docs/visuals/README.md` and `docs/visuals/icons/` are hand-authored
//! and deliberately outside the generated set.

#![cfg(feature = "report")]

use animsmith_testkit::docs_visuals;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Regenerate every visual into `out_dir` with the built CLI.
fn regenerate(out_dir: &Path) {
    let working_dir = repo_root().join(docs_visuals::WORKING_DIR);
    docs_visuals::write_docs_visuals(out_dir, |arguments| {
        let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
            .args(arguments)
            .current_dir(&working_dir)
            .output()
            .map_err(|error| format!("runs animsmith {}: {error}", arguments.join(" ")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "animsmith {} exited with {}: {}",
                arguments.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    })
    .expect("regenerates the documentation visuals");
}

/// Every generated file name, in manifest order.
fn generated_names() -> Vec<&'static str> {
    docs_visuals::REPORTS
        .iter()
        .map(|report| report.output)
        .chain(docs_visuals::CHARTS.iter().map(|chart| chart.output))
        .collect()
}

#[test]
fn committed_visuals_match_the_generator_output() {
    let temporary = tempfile::Builder::new()
        .prefix("animsmith-docs-visuals-")
        .tempdir()
        .expect("creates temp dir");
    regenerate(temporary.path());

    let committed = repo_root().join(docs_visuals::OUTPUT_DIR);
    for name in generated_names() {
        let expected = std::fs::read(temporary.path().join(name))
            .unwrap_or_else(|error| panic!("reads regenerated {name}: {error}"));
        let actual = std::fs::read(committed.join(name)).unwrap_or_else(|error| {
            panic!(
                "docs/visuals/{name} must be committed; run \
                 `cargo run -p animsmith --example gen_docs_visuals` ({error})"
            )
        });
        assert!(
            actual == expected,
            "docs/visuals/{name} does not match the generator output; \
             run `cargo run -p animsmith --example gen_docs_visuals`"
        );
    }
}

#[test]
fn the_committed_directory_holds_exactly_the_generated_set_plus_its_hand_authored_files() {
    let committed = repo_root().join(docs_visuals::OUTPUT_DIR);
    let present: BTreeSet<String> = std::fs::read_dir(&committed)
        .expect("lists docs/visuals/")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .into_string()
                .expect("utf-8 visual name")
        })
        .collect();
    let mut expected: BTreeSet<String> = generated_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    // The provenance page and the hand-drawn symptom icons are the only
    // files in this directory that no generator writes.
    expected.insert("README.md".to_owned());
    expected.insert("icons".to_owned());
    assert_eq!(
        present, expected,
        "docs/visuals/ must hold exactly the generated visuals plus README.md and icons/"
    );
}

#[test]
fn reports_embed_the_fixture_basename_rather_than_a_checkout_path() {
    let committed = repo_root().join(docs_visuals::OUTPUT_DIR);
    for report in docs_visuals::REPORTS {
        let html = std::fs::read_to_string(committed.join(report.output))
            .unwrap_or_else(|error| panic!("reads {}: {error}", report.output));
        assert!(
            !html.contains(docs_visuals::WORKING_DIR),
            "{} embeds a checkout-relative path; render it from the fixture directory",
            report.output
        );
        let input = report
            .arguments
            .iter()
            .find(|argument| argument.ends_with(".glb"))
            .expect("every report names a committed fixture");
        assert!(
            html.contains(&format!("\"file\":\"{input}\"")) || report.output.contains("comparison"),
            "{} must identify its input as {input}",
            report.output
        );
    }
}
