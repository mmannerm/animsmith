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

/// One rendered table cell: its text and the destinations it links to.
#[derive(Clone, Debug, Default, PartialEq)]
struct Cell {
    text: String,
    links: Vec<String>,
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
            Event::Text(text) | Event::Code(text) => {
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

fn tables(markdown: &str) -> Vec<Vec<Vec<String>>> {
    linked_tables(markdown)
        .into_iter()
        .map(|table| {
            table
                .into_iter()
                .map(|row| row.into_iter().map(|cell| cell.text).collect())
                .collect()
        })
        .collect()
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
        ("Animation troubleshooting", "animation-troubleshooting.md"),
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

    let troubleshooting = rendered_links(&markdown("docs/animation-troubleshooting.md"));
    for target in [
        "configuration-reference.md",
        "built-in-checks.md",
        "game-ready-clips.md#from-symptom-to-command",
    ] {
        assert!(
            troubleshooting
                .iter()
                .any(|(_, destination)| destination == target),
            "troubleshooting must route to canonical {target}: {troubleshooting:?}"
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
        "docs/animation-troubleshooting.md",
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
    assert!(
        temp.path()
            .join("troubleshooting-comparison.html")
            .is_file()
    );
}

/// Ownership and closing evidence live on the symptom page that opens
/// with the symptom, not in a separate matrix a reader has to correlate
/// with it. Each page's "Who fixes it" paragraph must still name the
/// owner it routes to and the evidence that closes its gate.
#[test]
fn every_symptom_page_names_its_owner_and_the_evidence_that_closes_its_gate() {
    for (page, owner, closing_evidence) in [
        (
            "docs/symptoms/pose-flickers.md",
            "the DCC owns the data",
            "re-lints clean",
        ),
        (
            "docs/symptoms/wrong-length.md",
            "the pipeline",
            "the engine plays the whole range",
        ),
        (
            "docs/symptoms/loop-pops.md",
            "DCC work",
            "target engine's graph",
        ),
        (
            "docs/symptoms/character-glides.md",
            "gameplay decides ownership",
            "an engine trial proves exactly one",
        ),
        (
            "docs/symptoms/blend-skate.md",
            "project work",
            "a playback capture covers the transitions",
        ),
        (
            "docs/symptoms/feet-slide.md",
            "DCC and runtime work",
            "the actual blend",
        ),
        (
            "docs/symptoms/limb-frozen.md",
            "the artist repairs the source rig",
            "required bones visibly move on the target character",
        ),
        (
            "docs/symptoms/identity-mismatch.md",
            "the pack owner and the pipeline that ingests it",
            "recorded source-to-target mapping",
        ),
        (
            "docs/symptoms/file-bloat.md",
            "the artist or the exporter settings",
            "the target engine's own scale, attachment and visual observation",
        ),
    ] {
        let prose = rendered_text(&markdown(page));
        let (_, ownership) = prose
            .split_once("Who fixes it:")
            .unwrap_or_else(|| panic!("{page} must carry a `Who fixes it` paragraph"));
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

/// The troubleshooting page is a router: every symptom it lists reaches
/// the page that owns it, and the two runtime problems with no symptom
/// page keep their own inspection, owner and closing evidence here.
#[test]
fn troubleshooting_routes_every_symptom_to_its_page_or_answers_it_in_place() {
    let page = markdown("docs/animation-troubleshooting.md");
    let tables = tables(&page);
    let table = tables.first().expect("troubleshooting routing table");
    assert_eq!(
        table.first(),
        Some(&vec!["What you see".to_owned(), "Where to go".to_owned()])
    );

    let destinations: BTreeSet<String> = rendered_links(&page)
        .into_iter()
        .map(|(_, destination)| destination)
        .collect();
    for symptom in [
        "symptoms/pose-flickers.md",
        "symptoms/wrong-length.md",
        "symptoms/loop-pops.md",
        "symptoms/character-glides.md",
        "symptoms/blend-skate.md",
        "symptoms/feet-slide.md",
        "symptoms/limb-frozen.md",
        "symptoms/identity-mismatch.md",
        "symptoms/file-bloat.md",
    ] {
        assert!(
            destinations.contains(symptom),
            "troubleshooting must route to {symptom}: {destinations:?}"
        );
    }
    let rows: Vec<&Vec<String>> = table.iter().skip(1).collect();
    assert_eq!(rows.len(), 9, "one routing row per symptom page: {table:?}");
    for row in rows {
        assert!(
            row.len() == 2 && !row[0].is_empty() && !row[1].is_empty(),
            "every routing row names what the reader sees and where to go: {row:?}"
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

/// Every registered check id, so a backticked word that is not a check —
/// a command, a config table — cannot be mistaken for one.
fn registered_check_ids() -> BTreeSet<String> {
    animsmith_core::all_checks()
        .iter()
        .map(|check| check.id().to_owned())
        .collect()
}

/// The `Symptom` → registered-check-ids map of a page's symptom table:
/// the first table whose leading header cell is exactly `Symptom`, read as
/// the symptom's own text against the code spans of its `Check(s)` cell.
fn symptom_check_ids(markdown: &str) -> BTreeMap<String, BTreeSet<String>> {
    let catalog = registered_check_ids();
    let mut rows = BTreeMap::new();
    let mut in_head = false;
    let mut is_symptom_table = false;
    let mut header = None::<String>;
    let mut cell = 0usize;
    let mut symptom = String::new();
    let mut checks = BTreeSet::new();

    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Table(_)) => {
                is_symptom_table = false;
                header = None;
                cell = 0;
            }
            Event::Start(Tag::TableHead) => {
                in_head = true;
                cell = 0;
            }
            Event::End(TagEnd::TableHead) => {
                in_head = false;
                is_symptom_table = header.as_deref() == Some("Symptom");
            }
            Event::Start(Tag::TableRow) => cell = 0,
            Event::Start(Tag::TableCell) => {
                cell += 1;
                if in_head && cell == 1 {
                    header = Some(String::new());
                }
                if is_symptom_table && !in_head && cell == 1 {
                    symptom.clear();
                    checks.clear();
                }
            }
            Event::Text(text) if in_head && cell == 1 => {
                header
                    .as_mut()
                    .expect("header cell is open")
                    .push_str(&text);
            }
            Event::Text(text) | Event::Code(text) if is_symptom_table && !in_head && cell == 1 => {
                symptom.push_str(&text);
            }
            Event::Code(code) if is_symptom_table && !in_head && cell == 2 => {
                if catalog.contains(code.as_ref()) {
                    checks.insert(code.into_string());
                }
            }
            Event::End(TagEnd::TableRow) if is_symptom_table && !in_head => {
                rows.insert(symptom.trim().to_owned(), std::mem::take(&mut checks));
            }
            Event::End(TagEnd::Table) if is_symptom_table => break,
            _ => {}
        }
    }
    rows
}

/// The symptom index and the game-ready guide each carry a symptom table,
/// and a reader who lands on either must be told the same thing. For every
/// symptom both name, the registered check ids in the `Check(s)` column
/// must be the same set — so neither table can quietly hand a check to a
/// symptom the other gives it to, the way the attachment-size check and
/// the export-bloat row could otherwise drift apart. The guide splits
/// families further than the index does; those narrower rows name symptoms
/// the index does not, and are not compared.
#[test]
fn both_symptom_tables_agree_about_which_checks_a_symptom_owns() {
    let index = symptom_check_ids(&markdown("docs/symptoms/README.md"));
    let guide = symptom_check_ids(&markdown("docs/game-ready-clips.md"));
    assert!(
        !index.is_empty() && !guide.is_empty(),
        "both tables are read"
    );

    let shared: Vec<&String> = index
        .keys()
        .filter(|name| guide.contains_key(*name))
        .collect();
    assert!(
        shared.len() >= 9,
        "the two tables must keep one symptom vocabulary, shared: {shared:?}"
    );
    for symptom in shared {
        assert_eq!(
            index[symptom], guide[symptom],
            "the symptom index and the game-ready guide disagree about {symptom:?}"
        );
    }

    // A check the catalog registers belongs to exactly one symptom in the
    // index, so the routing table cannot list it twice under two owners.
    let mut owner = BTreeMap::new();
    for (symptom, checks) in &index {
        for check in checks {
            if let Some(first) = owner.insert(check.clone(), symptom.clone()) {
                panic!("{check} is claimed by both {first:?} and {symptom:?}");
            }
        }
    }
}

#[test]
fn workflow_pages_are_current_state_routing_not_ticket_chronology_or_internal_notes() {
    for workflow_path in [
        "docs/animation-author-workflow.md",
        "docs/game-developer-intake-workflow.md",
        "docs/animation-troubleshooting.md",
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
