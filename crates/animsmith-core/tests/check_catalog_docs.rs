use animsmith_core::all_checks;
use std::collections::BTreeSet;

const README: &str = include_str!("../../../README.md");
const GAME_READY_CLIPS: &str = include_str!("../../../docs/game-ready-clips.md");

const EXEMPTED_REGISTERED_CHECK_IDS: &[&str] = &[];

const NON_CHECK_ID_LIKE_TOKENS: &[&str] = &[
    "animsmith",
    "animsmith-core",
    "animsmith-fbx",
    "animsmith-gltf",
    "animsmith-report",
    "fix",
    "humanoid",
    "measure",
    "mixamo",
    "transform",
    "ue-mannequin",
];

#[test]
fn docs_check_ids_match_the_registered_catalog() {
    let catalog = registered_check_ids();

    assert_catalog_ids(
        "README.md check tables",
        &readme_check_table_ids(),
        &catalog,
    );
    assert_catalog_ids(
        "docs/game-ready-clips.md symptom table",
        &guide_symptom_table_ids(),
        &catalog,
    );

    for (path, markdown) in [
        ("README.md", README),
        ("docs/game-ready-clips.md", GAME_READY_CLIPS),
    ] {
        let tokens = inline_code_tokens(markdown);
        let unknown_check_ids: Vec<_> = tokens
            .iter()
            .copied()
            .filter(|token| looks_like_check_id(token))
            .filter(|token| !catalog.contains(token))
            .filter(|token| !NON_CHECK_ID_LIKE_TOKENS.contains(token))
            .collect();
        assert!(
            unknown_check_ids.is_empty(),
            "{path} names check-like ids that are not registered: {unknown_check_ids:?}"
        );
    }
}

fn assert_catalog_ids(surface: &str, documented: &BTreeSet<&str>, catalog: &BTreeSet<&str>) {
    let missing: Vec<_> = catalog
        .iter()
        .copied()
        .filter(|id| !documented.contains(id))
        .filter(|id| !EXEMPTED_REGISTERED_CHECK_IDS.contains(id))
        .collect();
    assert!(
        missing.is_empty(),
        "{surface} does not document registered checks: {missing:?}"
    );

    let unknown: Vec<_> = documented
        .iter()
        .copied()
        .filter(|id| !catalog.contains(id))
        .collect();
    assert!(
        unknown.is_empty(),
        "{surface} documents checks that are not registered: {unknown:?}"
    );
}

fn registered_check_ids() -> BTreeSet<&'static str> {
    let checks = all_checks();
    let ids: Vec<_> = checks.iter().map(|check| check.id()).collect();
    let unique: BTreeSet<_> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len(), "duplicate registered check id");
    unique
}

fn readme_check_table_ids() -> BTreeSet<&'static str> {
    let tables = [
        markdown_table_after(README, "Mechanical checks run without project config:"),
        markdown_table_after(
            README,
            "Contract-aware checks use declared expectations and, where needed, rig roles:",
        ),
    ];
    tables
        .into_iter()
        .flat_map(|table| table.into_iter().skip(2))
        .filter_map(|row| table_cell(row, 0))
        .flat_map(inline_code_tokens)
        .collect()
}

fn guide_symptom_table_ids() -> BTreeSet<&'static str> {
    markdown_table_after(GAME_READY_CLIPS, "## From symptom to command")
        .into_iter()
        .skip(2)
        .filter_map(|row| table_cell(row, 1))
        .flat_map(inline_code_tokens)
        .collect()
}

fn markdown_table_after(markdown: &'static str, marker: &str) -> Vec<&'static str> {
    let mut lines = markdown.lines().skip_while(|line| *line != marker);
    let Some(_) = lines.next() else {
        panic!("missing marker: {marker}");
    };
    lines
        .skip_while(|line| line.trim().is_empty())
        .take_while(|line| line.trim_start().starts_with('|'))
        .collect()
}

fn table_cell(row: &str, index: usize) -> Option<&str> {
    row.trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .nth(index)
}

fn inline_code_tokens(markdown: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut in_fence = false;

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find('`') {
            let after_start = &rest[start + 1..];
            let Some(end) = after_start.find('`') else {
                break;
            };
            tokens.push(&after_start[..end]);
            rest = &after_start[end + 1..];
        }
    }

    tokens
}

fn looks_like_check_id(token: &str) -> bool {
    !token.starts_with('-')
        && !token.ends_with('-')
        && token.chars().all(|ch| ch.is_ascii_lowercase() || ch == '-')
}
