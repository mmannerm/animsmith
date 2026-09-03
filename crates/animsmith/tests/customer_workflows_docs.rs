//! Executable, parser-backed contract for the task-first customer workflows.
//!
//! This intentionally reads rendered Markdown events instead of matching source
//! lines: navigation and canonical links must survive normal Markdown syntax,
//! and the runnable examples must continue to agree with the current CLI.

use animsmith_testkit::docs_markdown::fenced_blocks;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::{Command, Output};

fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn markdown(path: &str) -> String {
    std::fs::read_to_string(repo(path)).unwrap_or_else(|error| panic!("reads {path}: {error}"))
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES
}

fn rendered_links(markdown: &str) -> Vec<(String, String)> {
    let mut links = Vec::new();
    let mut active = None;
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                active = Some((String::new(), dest_url.into_string()));
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((label, _)) = active.as_mut() {
                    label.push_str(&text);
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some(link) = active.take() {
                    links.push(link);
                }
            }
            _ => {}
        }
    }
    links
}

fn rendered_text(markdown: &str) -> String {
    let fragments: Vec<String> = Parser::new_ext(markdown, options())
        .filter_map(|event| match event {
            Event::Text(text) | Event::Code(text) => Some(text.into_string()),
            Event::SoftBreak | Event::HardBreak => Some("\n".to_owned()),
            _ => None,
        })
        .collect();
    fragments
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn headings(markdown: &str) -> Vec<String> {
    let mut headings = Vec::new();
    let mut active = None;
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Heading { .. }) => active = Some(String::new()),
            Event::Text(text) | Event::Code(text) => {
                if let Some(heading) = active.as_mut() {
                    heading.push_str(&text);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(heading) = active.take() {
                    headings.push(heading);
                }
            }
            _ => {}
        }
    }
    headings
}

/// One rendered table cell: its text, the destinations it links to, and
/// its code spans, which are how a cell names a check id rather than
/// merely mentioning a word.
#[derive(Clone, Debug, Default, PartialEq)]
struct Cell {
    text: String,
    links: Vec<String>,
    codes: Vec<String>,
}

fn linked_tables(markdown: &str) -> Vec<Vec<Vec<Cell>>> {
    let mut tables = Vec::new();
    let mut table = None;
    let mut row = None;
    let mut cell: Option<Cell> = None;
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Table(_)) => table = Some(Vec::new()),
            Event::Start(Tag::TableHead) => row = Some(Vec::new()),
            Event::Start(Tag::TableRow) => row = Some(Vec::new()),
            Event::Start(Tag::TableCell) => cell = Some(Cell::default()),
            Event::Start(Tag::Link { dest_url, .. }) => {
                if let Some(cell) = cell.as_mut() {
                    cell.links.push(dest_url.into_string());
                }
            }
            Event::Code(code) => {
                if let Some(cell) = cell.as_mut() {
                    cell.text.push_str(&code);
                    cell.codes.push(code.into_string());
                }
            }
            Event::Text(text) => {
                if let Some(cell) = cell.as_mut() {
                    cell.text.push_str(&text);
                }
            }
            Event::End(TagEnd::TableCell) => {
                row.as_mut()
                    .expect("table cell belongs to a row")
                    .push(cell.take().expect("table cell is active"));
            }
            Event::End(TagEnd::TableRow) => {
                table
                    .as_mut()
                    .expect("table row belongs to a table")
                    .push(row.take().expect("table row is active"));
            }
            Event::End(TagEnd::TableHead) => {
                table
                    .as_mut()
                    .expect("table head belongs to a table")
                    .push(row.take().expect("table head is active"));
            }
            Event::End(TagEnd::Table) => tables.push(table.take().expect("table is active")),
            _ => {}
        }
    }
    tables
}

/// The one table in `markdown` whose first heading cell is `column`.
fn table_headed_by(markdown: &str, column: &str) -> Vec<Vec<Cell>> {
    let mut matching = linked_tables(markdown).into_iter().filter(|table| {
        table
            .first()
            .and_then(|heading| heading.first())
            .is_some_and(|cell| cell.text == column)
    });
    let table = matching
        .next()
        .unwrap_or_else(|| panic!("no table is headed by {column}"));
    assert!(
        matching.next().is_none(),
        "exactly one table is headed by {column}"
    );
    table
}

/// The labelled blockquote lines a maintained report carries above its first
/// `##` section, as rendered text plus the bold spans of each line.
///
/// The animation-pack skill owns that grammar. `_bold_metadata_value` in
/// `.agents/skills/evaluate-animation-packs/scripts/validate_report.py` is the
/// authority `just animation-pack-skill` runs over every pair under
/// `docs/reports/`, and it recognizes nothing itself: it reads this same pinned
/// `pulldown-cmark` through the `animation_pack_markdown_ast` example, which a
/// Cargo test cannot borrow without nesting a Cargo build inside a test. So
/// this reads the same rendered structure with the same parser and asserts
/// nothing about which values are legal — a grammar or vocabulary change fails
/// the owning validator first, and this gate only projects what it accepted.
fn header_block(markdown: &str) -> Vec<(String, Vec<String>)> {
    let mut block = Vec::new();
    let mut quoted = 0usize;
    let mut line: Option<(String, Vec<String>)> = None;
    let mut bold: Option<String> = None;
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Heading { level, .. })
                if quoted == 0 && level != HeadingLevel::H1 =>
            {
                break;
            }
            Event::Start(Tag::BlockQuote(_)) => quoted += 1,
            Event::End(TagEnd::BlockQuote(_)) => quoted = quoted.saturating_sub(1),
            Event::Start(Tag::Paragraph) if quoted > 0 => line = Some((String::new(), Vec::new())),
            Event::Start(Tag::Strong) if line.is_some() => bold = Some(String::new()),
            Event::Text(text) | Event::Code(text) => {
                if let Some((rendered, _)) = line.as_mut() {
                    rendered.push_str(&text);
                }
                if let Some(bold) = bold.as_mut() {
                    bold.push_str(&text);
                }
            }
            Event::End(TagEnd::Strong) => {
                if let (Some(value), Some((_, spans))) = (bold.take(), line.as_mut()) {
                    spans.push(value);
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if let Some(rendered) = line.take() {
                    block.push(rendered);
                }
            }
            _ => {}
        }
    }
    block
}

/// The published value of one header-block field: the single bold span of the
/// `Label: **Value**` line, which may carry a ` — boundary` clause after it.
fn header_value(report: &str, markdown: &str, label: &str) -> String {
    let prefix = format!("{label}: ");
    let mut declaring = header_block(markdown)
        .into_iter()
        .filter(|(text, _)| text.starts_with(&prefix));
    let (text, bold) = declaring
        .next()
        .unwrap_or_else(|| panic!("{report} header block must declare {label}"));
    assert!(
        declaring.next().is_none(),
        "{report} header block must declare {label} once"
    );
    assert_eq!(bold.len(), 1, "{report} must declare one bold {label}");
    let declared = format!("{prefix}{}", bold[0]);
    assert!(
        text == declared || text.starts_with(&format!("{declared} — ")),
        "{report} must declare {label} as a bold value with an optional boundary clause: {text}"
    );
    bold[0].clone()
}

fn documented_commands(path: &str) -> Vec<(i32, String)> {
    fenced_blocks(&markdown(path), "console")
        .into_iter()
        .map(|block| {
            let (marker, command) = block
                .split_once('\n')
                .unwrap_or_else(|| panic!("{path} command block needs an exit marker: {block}"));
            let expected = marker
                .strip_prefix("# workflow-exit: ")
                .unwrap_or_else(|| panic!("{path} command marker is malformed: {marker}"))
                .parse::<i32>()
                .expect("workflow exit marker is an integer");
            assert!(
                (0..=2).contains(&expected) && command.contains("$ANIMSMITH"),
                "{path} command block has an invalid convention: {block}"
            );
            (expected, command.to_owned())
        })
        .collect()
}

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

fn has_numbered_tracker_reference(markdown: &str) -> bool {
    let words = words(&rendered_text(markdown));
    let named_reference = words.windows(2).any(|pair| {
        matches!(pair[0].as_str(), "issue" | "ticket" | "pr" | "milestone")
            && pair[1].chars().all(|character| character.is_numeric())
    }) || words.windows(3).any(|triple| {
        triple[0] == "pull"
            && triple[1] == "request"
            && triple[2].chars().all(|character| character.is_numeric())
    });
    named_reference || rendered_text_has_bare_hash_reference(markdown)
}

/// Detect shorthand such as `#600` only within rendered text/code source
/// spans. Link destinations are `Tag::Link` events, not text events, so an
/// ordinary `guide.md#600` destination cannot become tracker chronology.
fn rendered_text_has_bare_hash_reference(markdown: &str) -> bool {
    Parser::new_ext(markdown, options())
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Text(_) | Event::Code(_) => Some(&markdown[range]),
            _ => None,
        })
        .any(|source| {
            source
                .as_bytes()
                .windows(2)
                .enumerate()
                .any(|(offset, pair)| {
                    pair[0] == b'#'
                        && pair[1].is_ascii_digit()
                        && (offset == 0 || !source.as_bytes()[offset - 1].is_ascii_alphanumeric())
                })
        })
}

fn has_internal_or_historical_account(markdown: &str) -> bool {
    let prose = rendered_text(markdown).to_lowercase();
    [
        "we decided",
        "we chose",
        "our rationale",
        "internal discussion",
        "review feedback",
        "implementation reasoning",
        "previously",
        "formerly",
        "at the time",
    ]
    .iter()
    .any(|phrase| prose.contains(phrase))
}

fn has_historical_date_account(markdown: &str) -> bool {
    let words = words(&rendered_text(markdown));
    words.windows(2).any(|pair| {
        matches!(pair[0].as_str(), "in" | "since" | "before" | "after")
            && pair[1].len() == 4
            && pair[1].chars().all(|character| character.is_numeric())
    })
}

fn has_version_history_account(markdown: &str) -> bool {
    let prose = rendered_text(markdown).to_lowercase();
    [
        "version history",
        "earlier version",
        "prior version",
        "previous version",
        "changed in version",
    ]
    .iter()
    .any(|phrase| prose.contains(phrase))
}

#[test]
fn workflows_are_obvious_navigation_entry_points_with_canonical_routes() {
    let index_links = rendered_links(&markdown("docs/README.md"));
    for required in [
        (
            "For artists: from export to handoff",
            "animation-author-workflow.md",
        ),
        (
            "For game developers: from pack to engine gate",
            "game-developer-intake-workflow.md",
        ),
        ("Symptom index", "symptoms/README.md"),
        (
            "Commercial-pack evaluation guide",
            "commercial-pack-evaluations.md",
        ),
    ] {
        assert!(
            index_links
                .iter()
                .any(|(label, destination)| label == required.0 && destination == required.1),
            "docs index lacks navigation entry point {required:?}: {index_links:?}"
        );
    }

    let symptoms = rendered_links(&markdown("docs/symptoms/README.md"));
    for target in [
        "../configuration-reference.md",
        "../built-in-checks.md",
        "../game-ready-clips.md#the-readiness-ladder",
    ] {
        assert!(
            symptoms
                .iter()
                .any(|(_, destination)| destination == target),
            "the symptom index must route to canonical {target}: {symptoms:?}"
        );
    }

    let intake = rendered_links(&markdown("docs/game-developer-intake-workflow.md"));
    for target in [
        "engine-profile-bevy.md#revision-3-animationchannel-gate-support",
        "engine-profile-unity.md",
        "engine-profile-unreal.md",
        "engine-profile-godot.md",
        "engine-profile-gltf-runtime.md",
    ] {
        assert!(
            intake.iter().any(|(_, destination)| destination == target),
            "intake must route to maintained engine authority {target}: {intake:?}"
        );
    }

    let commercial = rendered_links(&markdown("docs/commercial-pack-evaluations.md"));
    assert!(
        commercial.iter().any(|(label, destination)| {
            label == "Technical issue register"
                && destination == "reports/protofactor-basic-locomotion.md#technical-issue-register"
        }),
        "commercial guide must use the exact maintained Technical issue register heading"
    );
}

#[test]
fn commercial_report_index_equals_the_maintained_on_disk_pairs() {
    let reports_dir = repo("docs/reports");
    let disk: BTreeSet<String> = std::fs::read_dir(&reports_dir)
        .expect("lists report directory")
        .map(|entry| entry.expect("report directory entry").file_name())
        .map(|name| name.into_string().expect("report name is UTF-8"))
        .filter(|name| name.ends_with(".md") && name != "README.md")
        .collect();
    let technical_on_disk: BTreeSet<String> = disk
        .iter()
        .filter(|name| !name.ends_with("-evidence.md"))
        .cloned()
        .collect();
    let appendices_on_disk: BTreeSet<String> = disk
        .iter()
        .filter(|name| name.ends_with("-evidence.md"))
        .cloned()
        .collect();
    assert!(
        !technical_on_disk.is_empty(),
        "maintained reports exist on disk"
    );
    for report in &technical_on_disk {
        let appendix = report.trim_end_matches(".md").to_owned() + "-evidence.md";
        assert!(
            appendices_on_disk.contains(&appendix),
            "maintained report {report} lacks on-disk appendix {appendix}"
        );
    }
    for appendix in &appendices_on_disk {
        let report = appendix.trim_end_matches("-evidence.md").to_owned() + ".md";
        assert!(
            technical_on_disk.contains(&report),
            "orphaned on-disk appendix {appendix} has no report {report}"
        );
    }

    let index: BTreeSet<String> = rendered_links(&markdown("docs/reports/README.md"))
        .into_iter()
        .map(|(_, destination)| destination)
        .filter(|destination| destination.ends_with(".md"))
        .collect();
    let technical_in_index: BTreeSet<String> = index
        .iter()
        .filter(|destination| !destination.ends_with("-evidence.md"))
        .cloned()
        .collect();
    let appendices_in_index: BTreeSet<String> = index
        .iter()
        .filter(|destination| destination.ends_with("-evidence.md"))
        .cloned()
        .collect();
    assert!(
        technical_in_index == technical_on_disk,
        "technical report index must equal maintained on-disk reports"
    );
    assert!(
        appendices_in_index == appendices_on_disk,
        "evidence appendix index must equal maintained on-disk appendices"
    );
    assert!(
        technical_on_disk
            .iter()
            .any(|path| path.starts_with("mixamo-"))
            && technical_on_disk
                .iter()
                .any(|path| path.starts_with("protofactor-")),
        "both maintained report families stay indexed"
    );
    for report in disk {
        let report_headings = headings(&markdown(&format!("docs/reports/{report}")));
        assert_eq!(
            report_headings
                .iter()
                .filter(|heading| heading.as_str() == "Changes between AnimSmith versions")
                .count(),
            1,
            "{report} needs one explicit historical-reader section"
        );
        for heading in report_headings {
            let lower = heading.to_lowercase();
            if ["history", "chronolog", "timeline", "version"]
                .iter()
                .any(|term| lower.contains(term))
            {
                assert_eq!(
                    heading, "Changes between AnimSmith versions",
                    "{report} must keep historical-reader content in Changes"
                );
            }
        }
    }
}

#[test]
fn commercial_report_scorecard_projects_each_published_header_block() {
    let index = markdown("docs/reports/README.md");
    let scorecard = table_headed_by(&index, "Pack");
    let canonical = table_headed_by(&index, "Technical report");
    assert_eq!(
        scorecard[0]
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<Vec<_>>(),
        [
            "Pack",
            "Technical verdict",
            "Evaluation completeness",
            "Confidence",
            "Evaluation date",
            "Current evaluator",
        ],
        "the scorecard's columns are exactly the report header-block fields"
    );

    let packs = |rows: &[Vec<Cell>], what: &str| -> Vec<(String, String)> {
        rows.iter()
            .map(|row| {
                let pack = row
                    .first()
                    .unwrap_or_else(|| panic!("{what} row has cells"));
                assert_eq!(
                    pack.links.len(),
                    1,
                    "{what} row must name exactly one report: {}",
                    pack.text
                );
                (pack.text.clone(), pack.links[0].clone())
            })
            .collect()
    };
    let scored = packs(&scorecard[1..], "scorecard");
    assert_eq!(
        scored,
        packs(&canonical[1..], "current-reports"),
        "the scorecard must carry one row per maintained pair, named and ordered \
         by the canonical current-reports table"
    );

    for (row, (_, report)) in scorecard[1..].iter().zip(&scored) {
        assert_eq!(row.len(), 6, "scorecard row shape for {report}");
        let published = markdown(&format!("docs/reports/{report}"));
        for (column, label) in [
            (1, "Technical verdict"),
            (2, "Evaluation completeness"),
            (3, "Confidence"),
            (4, "Evaluation date"),
            (5, "Current evaluator"),
        ] {
            assert_eq!(
                row[column].text,
                header_value(report, &published, label),
                "the scorecard must copy the published {label} of {report}"
            );
            assert!(
                row[column].links.is_empty(),
                "scorecard {label} of {report} is a copied value, not a route"
            );
        }
    }
}

#[test]
fn documented_command_fences_and_bevy_config_execute_exactly_as_rendered() {
    let bevy_workflow = markdown("docs/game-developer-intake-workflow.md");
    let bevy_prose = rendered_text(&bevy_workflow);
    for boundary in [
        "AnimSmith does not run Bevy",
        "read back a Bevy import",
        "not a prediction facet, are the evidence that closes them",
        "FBX source, then convert the candidate at the format boundary",
    ] {
        assert!(
            bevy_prose.contains(boundary),
            "Bevy boundary missing: {boundary}"
        );
    }
    let documented_config = fenced_blocks(&bevy_workflow, "toml");
    assert_eq!(documented_config.len(), 1, "one exact worked Bevy config");
    let documented_config: toml::Value =
        toml::from_str(&documented_config[0]).expect("documented Bevy TOML parses");
    let canonical_config: toml::Value = toml::from_str(
        &std::fs::read_to_string(repo("examples/bevy-v3.animsmith.toml"))
            .expect("reads canonical Bevy config"),
    )
    .expect("canonical Bevy config parses");
    assert_eq!(
        documented_config, canonical_config,
        "worked Bevy fence must be the canonical revision-3 config"
    );
    let engine = documented_config["engine"]
        .as_table()
        .expect("engine table");
    assert_eq!(
        engine.get("profile").and_then(toml::Value::as_str),
        Some("bevy")
    );
    assert_eq!(
        engine
            .get("profile_revision")
            .and_then(toml::Value::as_integer),
        Some(3)
    );
    assert_eq!(
        engine.get("engine_version").and_then(toml::Value::as_str),
        Some("0.19.0")
    );
    assert_eq!(
        engine.get("importer").and_then(toml::Value::as_str),
        Some("gltf-asset-loader")
    );
    let settings = engine
        .get("settings")
        .and_then(toml::Value::as_table)
        .expect("revision-3 settings table");
    assert_eq!(settings.len(), 3, "revision-3 settings stay complete");
    assert_eq!(
        settings
            .get("extension_handler_environment")
            .and_then(toml::Value::as_str),
        Some("bare_empty")
    );
    assert_eq!(
        settings
            .get("bevy_animation_feature")
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        settings
            .get("load_animations")
            .and_then(toml::Value::as_bool),
        Some(true)
    );

    let temp = tempfile::tempdir().expect("creates disposable command workspace");
    let bevy_config = temp.path().join("bevy-v3.animsmith.toml");
    std::fs::write(&bevy_config, &fenced_blocks(&bevy_workflow, "toml")[0])
        .expect("materializes the exact documented Bevy config");
    let asset_dir = repo("examples/assets");
    let config_dir = repo("examples");
    let fbx = repo("crates/animsmith-fbx/testdata/rigged_triangle.fbx");
    for page in [
        "docs/animation-author-workflow.md",
        "docs/game-developer-intake-workflow.md",
        "docs/symptoms/README.md",
    ] {
        for (expected, command) in documented_commands(page) {
            let output = Command::new("sh")
                .args(["-eu", "-c", &command])
                .env("ANIMSMITH", env!("CARGO_BIN_EXE_animsmith"))
                .env("ASSET_DIR", &asset_dir)
                .env("CONFIG_DIR", &config_dir)
                .env("WORK_DIR", temp.path())
                .env("FBX_FIXTURE", &fbx)
                .env("BEVY_CONFIG", &bevy_config)
                .output()
                .unwrap_or_else(|error| panic!("runs documented command from {page}: {error}"));
            assert_eq!(
                output.status.code(),
                Some(expected),
                "documented command from {page} diverged:\n{command}\nstderr:\n{}",
                stderr(&output)
            );
            if page == "docs/game-developer-intake-workflow.md" && expected == 1 {
                assert!(
                    String::from_utf8_lossy(&output.stdout)
                        .contains("required_prediction_unavailable"),
                    "the exact Bevy command must retain the non-engine survival boundary"
                );
            }
        }
    }
    assert!(temp.path().join("candidate.glb").is_file());
    assert!(temp.path().join("author-comparison.html").is_file());
    assert!(temp.path().join("symptom-comparison.html").is_file());
}

/// Every symptom page, read from the directory rather than from a list a
/// gate keeps, so a tenth page is gated the moment it is committed.
/// `start_docs.rs` reads the same directory for the same reason.
fn symptom_pages() -> BTreeSet<String> {
    let pages: BTreeSet<String> = std::fs::read_dir(repo("docs/symptoms"))
        .expect("lists the symptom pages")
        .filter_map(|entry| {
            let name = entry.expect("directory entry").file_name();
            let name = name.into_string().expect("utf-8 page name");
            (name.ends_with(".md") && name != "README.md").then_some(name)
        })
        .collect();
    assert!(
        !pages.is_empty(),
        "docs/symptoms/ must publish symptom pages"
    );
    pages
}

/// The rendered text of the one paragraph that opens with `Who fixes it:`.
///
/// Reading everything after that phrase would let a later paragraph
/// contradict the ownership this one states and still satisfy the gate.
fn ownership_paragraph(page: &str) -> String {
    let markdown = markdown(page);
    let mut found: Option<String> = None;
    let mut current: Option<String> = None;
    for event in Parser::new_ext(&markdown, options()) {
        match event {
            Event::Start(Tag::Paragraph) => current = Some(String::new()),
            Event::Text(text) | Event::Code(text) => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph.push(' ');
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if let Some(paragraph) = current.take()
                    && paragraph.trim_start().starts_with("Who fixes it:")
                {
                    assert!(
                        found.is_none(),
                        "{page} must carry exactly one `Who fixes it` paragraph"
                    );
                    found = Some(paragraph);
                }
            }
            _ => {}
        }
    }
    let paragraph = found.unwrap_or_else(|| panic!("{page} must carry a `Who fixes it` paragraph"));
    paragraph.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Ownership and closing evidence live on the symptom page that opens
/// with the symptom, not in a separate matrix a reader has to correlate
/// with it. Each page's "Who fixes it" paragraph must name the owner it
/// routes to and the evidence that closes its gate — inside that
/// paragraph, so a later one cannot contradict it and still pass.
#[test]
fn every_symptom_page_names_its_owner_and_the_evidence_that_closes_its_gate() {
    let expected = [
        (
            "pose-flickers.md",
            "the DCC owns the data",
            "re-lints clean",
        ),
        (
            "wrong-length.md",
            "the pipeline",
            "the engine plays the whole range",
        ),
        ("loop-pops.md", "DCC work", "target engine's graph"),
        (
            "character-glides.md",
            "gameplay decides ownership",
            "an engine trial proves exactly one",
        ),
        (
            "blend-skate.md",
            "project work",
            "a playback capture covers the transitions",
        ),
        ("feet-slide.md", "DCC and runtime work", "the actual blend"),
        (
            "limb-frozen.md",
            "the artist repairs the source rig",
            "required bones visibly move on the target character",
        ),
        (
            "identity-mismatch.md",
            "the pack owner and the pipeline that ingests it",
            "recorded source-to-target mapping",
        ),
        (
            "file-bloat.md",
            "the artist or the exporter settings",
            "the target engine's own scale, attachment and visual observation",
        ),
    ];
    assert_eq!(
        expected
            .iter()
            .map(|(page, _, _)| (*page).to_owned())
            .collect::<BTreeSet<String>>(),
        symptom_pages(),
        "every symptom page states its owner and closing evidence here"
    );

    for (page, owner, closing_evidence) in expected {
        let page = format!("docs/symptoms/{page}");
        let ownership = ownership_paragraph(&page);
        assert!(
            ownership.contains(owner),
            "{page} must route ownership to {owner:?}: {ownership}"
        );
        assert!(
            ownership.contains(closing_evidence),
            "{page} must state the evidence that closes its gate ({closing_evidence:?}): \
             {ownership}"
        );
    }
}

/// The byte range of the `## <title>` section: from that heading's own
/// start to the next heading of the same level or higher.
///
/// Read from heading events rather than from a `find("## ")`, so a `##`
/// inside a fenced block or a differently spelled heading cannot open or
/// close a section.
fn section_span(markdown: &str, title: &str) -> Option<std::ops::Range<usize>> {
    let mut start: Option<usize> = None;
    let mut heading: Option<usize> = None;
    let mut text = String::new();
    for (event, range) in Parser::new_ext(markdown, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                heading = Some(range.start);
                text.clear();
            }
            Event::Text(part) | Event::Code(part) if heading.is_some() => text.push_str(&part),
            Event::End(TagEnd::Heading(level)) => {
                let offset = heading.take().expect("a heading ends after it starts");
                match start {
                    None if level == HeadingLevel::H2 && text.trim() == title => {
                        start = Some(offset);
                    }
                    Some(start) if level <= HeadingLevel::H2 => return Some(start..offset),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    start.map(|start| start..markdown.len())
}

/// The `src` of every `<iframe>` the page **renders**, in document
/// order. A frame inside a fenced block is an example rather than
/// something a reader is shown, so it cannot satisfy a promise here.
fn frames(markdown: &str) -> Vec<String> {
    animsmith_testkit::docs_markdown::rendered_html_references(markdown, &[("iframe", "src")])
        .into_iter()
        .map(|(_, source)| source)
        .collect()
}

/// The committed fixture a page is about: the one named by the first
/// `animsmith` command in its own transcripts that runs one. A page opens
/// with commands that take no clip, so this is the first *fixture*
/// argument rather than the first command.
///
/// Taking it from the page rather than from a table beside the gate is
/// what binds a page to its own clip: a page that embedded the report of
/// somebody else's fixture would still be showing a real measurement of a
/// real clip, and only the page's own commands say which clip that should
/// be. It is read through the shared transcript reader and only from the
/// command line, so a fixture named in quoted *output* — or by a
/// `cargo` line the gate never runs — cannot claim the page.
fn documented_fixture(markdown: &str, page: &str) -> Option<String> {
    fenced_blocks(markdown, "console")
        .into_iter()
        .flat_map(|block| animsmith_testkit::docs_transcripts::documented_commands(&block, page))
        .filter(|documented| documented.command.starts_with("animsmith "))
        .find_map(|documented| {
            documented
                .command
                .split_whitespace()
                .filter_map(|argument| argument.strip_prefix("examples/assets/"))
                .find(|name| name.ends_with(".glb"))
                .map(str::to_owned)
        })
}

/// The check ids a page names in its header, above its first `##`
/// section.
fn header_checks(markdown: &str) -> BTreeSet<String> {
    let registered = registered_check_ids();
    let mut named = BTreeSet::new();
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                ..
            }) => break,
            Event::Code(code) if registered.contains(code.as_ref()) => {
                named.insert(code.into_string());
            }
            _ => {}
        }
    }
    named
}

/// The findings a committed single-clip report embeds, in the order the
/// viewer lists them and `#finding=N` indexes them.
fn report_findings(visual: &std::path::Path) -> Vec<serde_json::Value> {
    const MARKER: &str = "<script type=\"application/json\" id=\"report-data\">";
    let html = std::fs::read_to_string(visual)
        .unwrap_or_else(|error| panic!("reads {}: {error}", visual.display()));
    let start = html
        .find(MARKER)
        .unwrap_or_else(|| panic!("{} carries its report data", visual.display()))
        + MARKER.len();
    let end = html[start..]
        .find("</script>")
        .unwrap_or_else(|| panic!("{} closes its report data", visual.display()))
        + start;
    let data: serde_json::Value =
        serde_json::from_str(&html[start..end]).expect("report data is JSON");
    data["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("{} lists findings", visual.display()))
        .clone()
}

/// The first report a symptom page embeds is its own defective clip's,
/// not a picture of that clip already repaired.
///
/// Every page embeds, inside its "What AnimSmith measures" section, the
/// single-clip report of the fixture its own transcripts run, and that is
/// the first report any of its frames carry; a comparison, which
/// shows two clips and which an absent finding on one side can flatter,
/// comes later under its own heading. This is about report documents
/// rather than still pictures: `loop-pops.md` opens the same section with
/// two foot-height charts, which is the page reading well and not a
/// repaired clip standing in for the defective one.
///
/// Where the frame deep-links a finding, that finding is the first one of
/// its check on the clip, and its check is one the page's header names —
/// so the viewer opens on the symptom the page is about rather than on
/// whatever the renderer happens to list first.
#[test]
fn the_first_report_a_symptom_page_embeds_is_its_defective_clips_own() {
    let visuals = repo("docs/visuals");
    let fixtures = repo(animsmith_testkit::docs_visuals::WORKING_DIR);
    let mut unbound: Vec<String> = Vec::new();
    for page in symptom_pages() {
        let path = format!("docs/symptoms/{page}");
        let markdown = markdown(&path);
        let Some(fixture) = documented_fixture(&markdown, &path) else {
            assert!(
                frames(&markdown).is_empty(),
                "{path} runs no committed fixture of its own, so it must embed no report: {:?}",
                frames(&markdown)
            );
            unbound.push(page);
            continue;
        };
        assert!(
            fixtures.join(&fixture).is_file(),
            "{path} runs {fixture}, which examples/assets/ does not hold"
        );

        let section = section_span(&markdown, "What AnimSmith measures")
            .unwrap_or_else(|| panic!("{path} must carry a `What AnimSmith measures` section"));
        assert!(
            frames(&markdown[..section.start]).is_empty(),
            "{path} embeds a report above its `What AnimSmith measures` heading: {:?}",
            frames(&markdown[..section.start])
        );
        let embedded = frames(&markdown[section.clone()]);
        let source = embedded.first().unwrap_or_else(|| {
            panic!("{path} embeds no report in its `What AnimSmith measures` section")
        });

        let name = source
            .split('#')
            .next()
            .and_then(|path| path.strip_prefix("../visuals/"))
            .unwrap_or_else(|| panic!("{path} must embed a document under docs/visuals: {source}"));
        assert!(
            visuals.join(name).is_file(),
            "{path} embeds {name}, which docs/visuals/ does not hold"
        );
        let command = animsmith_testkit::docs_visuals::COMMANDS
            .iter()
            .find(|command| command.output == name)
            .unwrap_or_else(|| panic!("{path} embeds {name}, which no generator writes"));
        assert!(
            !command.arguments.contains(&"--compare-after"),
            "the first report {path} embeds is {name}, which is a two-clip comparison: a page \
             embeds the defective clip's own report first and shows a comparison under a \
             later heading"
        );
        assert_eq!(
            command
                .arguments
                .iter()
                .copied()
                .filter(|argument| argument.ends_with(".glb"))
                .collect::<Vec<_>>(),
            vec![fixture.as_str()],
            "{path} runs {fixture} and embeds {name}, which is a report of another clip"
        );

        // The deep link scrubs the viewer to `findings[N]`, so an index
        // the report no longer has opens on nothing, and one that lands
        // on a different check opens on the wrong symptom.
        if let Some(index) = source
            .split(['#', '&'])
            .find_map(|option| option.strip_prefix("finding="))
        {
            let index: usize = index
                .parse()
                .unwrap_or_else(|error| panic!("{path}: unreadable finding index: {error}"));
            let findings = report_findings(&visuals.join(name));
            let finding = findings.get(index).unwrap_or_else(|| {
                panic!(
                    "{path} opens {name} on finding {index}, which it does not have \
                     ({} finding(s))",
                    findings.len()
                )
            });
            let check = finding["check"]
                .as_str()
                .expect("a finding names its check");
            assert!(
                header_checks(&markdown).contains(check),
                "{path} opens {name} on a {check:?} finding, which its header does not name"
            );
            assert_eq!(
                findings
                    .iter()
                    .position(|row| row["check"] == finding["check"]),
                Some(index),
                "{path} opens {name} on finding {index}, which is not the first {check:?} \
                 finding on that clip"
            );
        }
    }
    assert_eq!(
        unbound,
        ["identity-mismatch.md"],
        "identity is the one symptom answered by `inspect` and a collection manifest rather \
         than by a measurement of one clip; every other page runs the clip it is about"
    );
}

/// The symptom index is the router: every page in its directory has a row
/// that reaches it, every row reaches a page in that directory, and the
/// runtime problems with no page of their own — the two that are not
/// findings about a clip, and Bevy's silent loader drop — are answered on
/// the index itself.
#[test]
fn the_symptom_index_routes_every_page_and_answers_what_is_not_a_clip() {
    let page = markdown("docs/symptoms/README.md");
    let table = table_headed_by(&page, "Symptom");
    assert_eq!(
        table
            .first()
            .map(|row| row.iter().map(|cell| cell.text.clone()).collect::<Vec<_>>()),
        Some(vec![
            "Symptom".to_owned(),
            "Check(s)".to_owned(),
            "Repair / transform".to_owned(),
            "Config surface".to_owned(),
            "Who fixes it".to_owned(),
            "Page".to_owned(),
        ])
    );

    let pages = symptom_pages();
    let routed: BTreeSet<String> = table
        .iter()
        .skip(1)
        .flat_map(|row| row.last().expect("every row names a page").links.clone())
        .map(|destination| {
            destination
                .split('#')
                .next()
                .expect("a destination has a path")
                .to_owned()
        })
        .collect();
    assert_eq!(
        routed, pages,
        "the index routes exactly the pages of its own directory"
    );
    for row in table.iter().skip(1) {
        assert!(
            row.len() == 6 && row.iter().all(|cell| !cell.text.is_empty()),
            "every row states the symptom, checks, repair, config, owner and page: {row:?}"
        );
    }

    let prose = rendered_text(&page);
    for (symptom, inspection, ownership, closure) in [
        (
            "A loader error or an AnimSmith refusal",
            "engine-addressability",
            "belong to the engine project",
            "engine-observed load evidence",
        ),
        (
            "A clip exists but cannot be addressed in-engine",
            "generate addressability",
            "are engine code",
            "resolved runtime asset",
        ),
        (
            "Animations vanish in Bevy with no lint error",
            "No content finding exists",
            "load_animations",
            "revision-3 gate",
        ),
    ] {
        assert!(prose.contains(symptom), "{symptom} keeps its own answer");
        for required in [inspection, ownership, closure] {
            assert!(
                prose.contains(required),
                "{symptom} must keep {required:?}: {prose}"
            );
        }
    }
}

/// The pages that carry the shared "where you are" strip, and the stage each
/// one is.
const STRIP_PAGES: [(&str, &str); 4] = [
    ("docs/animation-author-workflow.md", "Artist export"),
    ("docs/declaring-the-contract.md", "Contract"),
    ("docs/game-developer-intake-workflow.md", "Developer intake"),
    ("docs/pipeline-scenarios.md", "CI gate"),
];

/// The strip as `(label, destination, is the reader's own stage)`, read from
/// the one paragraph that opens with `Where you are:`.
fn stage_strip(markdown: &str) -> Vec<(String, String, bool)> {
    let (mut in_strip, mut strong, mut link) = (false, false, None);
    let mut stages = Vec::new();
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::End(TagEnd::Paragraph) => in_strip = false,
            Event::Text(text) if text.starts_with("Where you are:") => in_strip = true,
            Event::Start(Tag::Strong) if in_strip => strong = true,
            Event::End(TagEnd::Strong) if in_strip => strong = false,
            Event::Start(Tag::Link { dest_url, .. }) if in_strip => {
                link = Some((String::new(), dest_url.into_string(), strong));
            }
            Event::Text(text) => {
                if let Some((label, _, _)) = link.as_mut() {
                    label.push_str(&text);
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some(stage) = link.take() {
                    stages.push(stage);
                }
            }
            _ => {}
        }
    }
    stages
}

/// The strip is four hand-written copies of one line, so nothing but a gate
/// keeps them the same line. A stage pointing at a different page, a dropped
/// stage, or a reordering is the drift that would leave a reader on the wrong
/// map; only which stage is bold may differ, and it must be the page's own.
#[test]
fn the_where_you_are_strip_is_the_same_five_stages_on_every_page_that_carries_it() {
    let canonical: Vec<(String, String)> = stage_strip(&markdown(STRIP_PAGES[0].0))
        .into_iter()
        .map(|(label, destination, _)| (label, destination))
        .collect();
    assert_eq!(
        canonical.len(),
        5,
        "the strip names the five pipeline stages: {canonical:?}"
    );

    for (page, stage) in STRIP_PAGES {
        let strip = stage_strip(&markdown(page));
        assert_eq!(
            strip
                .iter()
                .map(|(label, destination, _)| (label.clone(), destination.clone()))
                .collect::<Vec<_>>(),
            canonical,
            "{page} must carry the same stages, in order, at the same targets"
        );
        let bold: Vec<&String> = strip
            .iter()
            .filter(|(_, _, current)| *current)
            .map(|(label, _, _)| label)
            .collect();
        assert_eq!(
            bold,
            vec![&stage.to_owned()],
            "{page} must mark exactly its own stage as the reader's place"
        );
    }
}

/// Every registered check id, so a backticked word that is not a check —
/// a command, a config table — cannot be mistaken for one.
fn registered_check_ids() -> BTreeSet<String> {
    animsmith_core::all_checks()
        .iter()
        .map(|check| check.id().to_owned())
        .collect()
}

/// The symptom index is one table, so a check belongs to exactly one
/// symptom in it: two rows claiming the same check is the reader-visible
/// contradiction the split tables used to hide. Completeness — that every
/// registered check appears at all — is `check_catalog_docs.rs`.
#[test]
fn no_check_is_claimed_by_two_symptoms_in_the_index() {
    let catalog = registered_check_ids();
    let table = table_headed_by(&markdown("docs/symptoms/README.md"), "Symptom");
    let mut owner: BTreeMap<String, String> = BTreeMap::new();
    let mut claimed = 0usize;
    for row in table.iter().skip(1) {
        let symptom = row[0].text.trim().to_owned();
        for code in &row[1].codes {
            if !catalog.contains(code) {
                continue;
            }
            if let Some(first) = owner.insert(code.clone(), symptom.clone()) {
                panic!("{code} is claimed by both {first:?} and {symptom:?}");
            }
            claimed += 1;
        }
    }
    assert_eq!(
        claimed,
        catalog.len(),
        "every registered check is claimed exactly once: {owner:?}"
    );
}

#[test]
fn workflow_pages_are_current_state_routing_not_ticket_chronology_or_internal_notes() {
    for workflow_path in [
        "docs/animation-author-workflow.md",
        "docs/game-developer-intake-workflow.md",
        "docs/symptoms/README.md",
        "docs/commercial-pack-evaluations.md",
    ] {
        let page = markdown(workflow_path);
        assert!(
            !has_numbered_tracker_reference(&page),
            "{workflow_path} must not route readers through numbered tracker chronology"
        );
        assert!(
            !has_internal_or_historical_account(&page),
            "{workflow_path} must not contain internal rationale or a historical account"
        );
        assert!(
            !has_historical_date_account(&page),
            "{workflow_path} must not contain a dated historical account"
        );
        assert!(
            !has_version_history_account(&page),
            "{workflow_path} must not contain version-history narration outside reports"
        );
        for (_, destination) in rendered_links(&page) {
            assert!(
                !destination.contains("/issues/") && !destination.contains("/pull/"),
                "{workflow_path} must not contain tracker links: {destination}"
            );
        }
        for heading in headings(&page) {
            let heading = heading.to_lowercase();
            assert!(
                ![
                    "history",
                    "chronolog",
                    "timeline",
                    "change log",
                    "internal",
                    "analysis"
                ]
                .iter()
                .any(|term| heading.contains(term)),
                "{workflow_path} has a non-current-state section: {heading}"
            );
        }
    }
}

#[test]
fn current_state_policy_allows_customer_issue_register_but_rejects_neutral_heading_violations() {
    assert!(
        !has_numbered_tracker_reference("## Technical issue register\n\nRoute the owner."),
        "the customer-facing maintained heading is not tracker chronology"
    );
    assert!(has_numbered_tracker_reference(
        "## Intake\n\nIssue 600 changed this workflow."
    ));
    assert!(has_numbered_tracker_reference(
        "## Intake\n\nPull request 601 changed this workflow."
    ));
    assert!(has_numbered_tracker_reference(
        "## Intake\n\nRelated: #600."
    ));
    assert!(has_numbered_tracker_reference(
        "## Intake\n\nSupersedes #600."
    ));
    assert!(
        !has_numbered_tracker_reference(
            "## Intake\n\n[Revision-three guide](engine-profile-bevy.md#revision-3-animationchannel-gate-support)."
        ),
        "an anchor destination is navigation, not tracker chronology"
    );
    assert!(
        !has_numbered_tracker_reference("## Current intake\n\nRoute the owner."),
        "ordinary Markdown headings are not tracker chronology"
    );
    assert!(has_internal_or_historical_account(
        "## Intake\n\nWe chose this flow after internal discussion."
    ));
    assert!(has_internal_or_historical_account(
        "## Intake\n\nPreviously the loader behaved differently."
    ));
    assert!(has_historical_date_account(
        "## Intake\n\nIn 2024 the loader behaved differently."
    ));
    assert!(has_version_history_account(
        "## Intake\n\nThe prior version behaved differently."
    ));
}
