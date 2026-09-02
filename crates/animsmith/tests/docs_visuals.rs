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

use animsmith_core::sha256_hex;
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

    // No manifest can regenerate a drawing, so the provenance page beside
    // them is the icons' inventory: every icon is accounted for there, and
    // the directory holds nothing but drawings.
    let provenance = std::fs::read_to_string(committed.join("README.md"))
        .expect("reads the visuals provenance page");
    let icons: BTreeSet<String> = std::fs::read_dir(committed.join("icons"))
        .expect("lists docs/visuals/icons/")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .into_string()
                .expect("utf-8 icon name")
        })
        .collect();
    assert!(
        !icons.is_empty(),
        "docs/visuals/icons/ holds the hand-drawn symptom icons"
    );
    for icon in &icons {
        assert!(
            icon.ends_with(".svg"),
            "docs/visuals/icons/ holds drawings only, found {icon}"
        );
        assert!(
            provenance.contains(&format!("icons/{icon}")),
            "docs/visuals/README.md must account for the hand-authored icons/{icon}"
        );
    }
}

/// Every committed report identifies each fixture it was rendered from,
/// and none of them embeds a path from the checkout that rendered it. A
/// single-input document records the basename the command was given; a
/// comparison document records each side by content identity instead, so
/// its two fixtures are named by their digests. Either way both sides
/// are pinned to the committed bytes.
#[test]
fn reports_identify_every_fixture_input_and_embed_no_checkout_path() {
    let root = repo_root();
    let committed = root.join(docs_visuals::OUTPUT_DIR);
    let fixtures = root.join(docs_visuals::WORKING_DIR);
    for report in docs_visuals::REPORTS {
        let html = std::fs::read_to_string(committed.join(report.output))
            .unwrap_or_else(|error| panic!("reads {}: {error}", report.output));
        assert!(
            !html.contains(docs_visuals::WORKING_DIR),
            "{} embeds a checkout-relative path; render it from the fixture directory",
            report.output
        );

        let inputs: Vec<&str> = report
            .arguments
            .iter()
            .copied()
            .filter(|argument| argument.ends_with(".glb"))
            .collect();
        assert!(
            !inputs.is_empty(),
            "{} must name a committed fixture",
            report.output
        );
        for input in inputs {
            let bytes = std::fs::read(fixtures.join(input))
                .unwrap_or_else(|error| panic!("reads the {input} fixture: {error}"));
            let digest = sha256_hex(&bytes);
            assert!(
                html.contains(&format!("\"file\":\"{input}\""))
                    || html.contains(&format!("\"sha256\":\"{digest}\"")),
                "{} must identify its input {input} by name or by content identity",
                report.output
            );
        }
    }
}

/// Every colour a committed picture paints resolves through the
/// documentation theme's `--as-*` token, with the standalone value as its
/// fallback.
///
/// The site chooses light or dark by a class on the page, which an SVG's
/// own `prefers-color-scheme` cannot see: a reader who picks a theme
/// different from the operating system's would otherwise get a dark chart
/// on a light page. The build inlines these files into the pages that
/// show them, so the page's token wins there and the fallback still reads
/// correctly on GitHub or in a browser tab.
#[test]
fn every_committed_picture_paints_through_a_theme_token() {
    let visuals = repo_root().join(docs_visuals::OUTPUT_DIR);
    let mut checked = 0usize;
    for directory in [visuals.clone(), visuals.join("icons")] {
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("lists {}: {error}", directory.display()))
        {
            let path = entry.expect("directory entry").path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("svg") {
                continue;
            }
            let svg = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("reads {}: {error}", path.display()));
            let name = path.file_name().expect("a picture has a name").display();
            assert!(
                svg.contains("var(--as-"),
                "{name} paints nothing through a theme token"
            );
            assert_eq!(
                untokenized_colours(&svg),
                Vec::<String>::new(),
                "{name} paints a colour the page's theme cannot reach"
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 6,
        "every committed picture is checked, saw {checked}"
    );
}

/// Every colour literal in `svg` that is not the fallback of an `--as-*`
/// token, in document order.
fn untokenized_colours(svg: &str) -> Vec<String> {
    let mut loose = Vec::new();
    for (offset, _) in svg.match_indices('#') {
        let hex: String = svg[offset + 1..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        // `#chart-…` and `#icon-…` are selectors, not colours: a colour
        // is three, four, six or eight hexadecimal digits.
        if !matches!(hex.len(), 3 | 4 | 6 | 8) {
            continue;
        }
        let token = svg[..offset]
            .trim_end()
            .strip_suffix(',')
            .and_then(|head| head.rsplit_once("var(").map(|(_, token)| token));
        if !token.is_some_and(|token| {
            token.starts_with("--as-") && !token.contains(['(', ')', ',', ' '])
        }) {
            loose.push(format!("#{hex}"));
        }
    }
    loose
}
