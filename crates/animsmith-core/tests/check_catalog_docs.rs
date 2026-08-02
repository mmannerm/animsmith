use animsmith_core::{all_checks, mechanical_checks};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const NON_CHECK_ID_LIKE_TOKENS: &[&str] = &[
    "animsmith",
    "animsmith-core",
    "animsmith-fbx",
    "animsmith-gltf",
    "animsmith-report",
    "convert",
    "diff",
    "fix",
    "humanoid",
    "inspect",
    "lint",
    "measure",
    "mixamo",
    "report",
    "transform",
    "ue-mannequin",
];

const PARTIAL_CHECK_ID_DOCS: &[&str] = &["docs/pipeline-scenarios.md"];

#[test]
fn docs_check_ids_match_the_registered_catalog() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = source_workspace_root(manifest_dir) else {
        // Published crates intentionally exclude repository-level docs.
        return;
    };
    let readme = read_workspace_doc(&workspace_root, "README.md");
    let game_ready_clips = read_workspace_doc(&workspace_root, "docs/game-ready-clips.md");
    let catalog = registered_check_ids();
    let mechanical = registered_mechanical_check_ids();
    let contract_aware: BTreeSet<_> = catalog.difference(&mechanical).copied().collect();

    assert_exact_ids(
        "README.md Mechanical checks table",
        &readme_mechanical_check_table_ids(&readme),
        &mechanical,
    );
    assert_exact_ids(
        "README.md Contract-aware checks table",
        &readme_contract_aware_check_table_ids(&readme),
        &contract_aware,
    );
    assert_exact_ids(
        "docs/game-ready-clips.md symptom table",
        &guide_symptom_table_ids(&game_ready_clips),
        &catalog,
    );
    assert_exact_ids(
        "docs/game-ready-clips.md File-ready level",
        &guide_file_ready_check_ids(&game_ready_clips, &catalog),
        &mechanical,
    );

    for (path, markdown) in [
        ("README.md", readme.as_str()),
        ("docs/game-ready-clips.md", game_ready_clips.as_str()),
    ] {
        let tokens = inline_code_tokens(markdown);
        let documented: BTreeSet<_> = tokens
            .iter()
            .copied()
            .filter(|token| catalog.contains(token))
            .collect();
        assert_eq!(
            documented, catalog,
            "{path} inline-code scan must see the registered check ids"
        );

        assert_no_unknown_check_ids(path, markdown, &catalog);
    }

    for path in PARTIAL_CHECK_ID_DOCS {
        let markdown = read_workspace_doc(&workspace_root, path);
        assert_no_unknown_check_ids(path, &markdown, &catalog);
    }
}

#[test]
fn source_workspace_detection_has_a_positive_checkout_control() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let detected = source_workspace_root(manifest_dir);
    if manifest_dir.join(".cargo_vcs_info.json").is_file() {
        assert!(detected.is_none(), "published packages must skip repo docs");
        return;
    }

    let expected = manifest_dir.join("../..");
    if expected.join("docs/output.md").is_file() {
        assert_eq!(
            detected.as_deref(),
            Some(expected.as_path()),
            "the exact source checkout must enforce its catalog docs"
        );
    }
}

fn source_workspace_root(manifest_dir: &Path) -> Option<PathBuf> {
    if manifest_dir.join(".cargo_vcs_info.json").is_file() {
        return None;
    }
    let workspace_root = manifest_dir.join("../..");
    let current_manifest = manifest_dir.join("Cargo.toml").canonicalize().ok()?;
    let workspace_manifest = workspace_root
        .join("crates/animsmith-core/Cargo.toml")
        .canonicalize()
        .ok()?;
    (current_manifest == workspace_manifest).then_some(workspace_root)
}

fn read_workspace_doc(workspace_root: &Path, relative_path: &str) -> String {
    let path = workspace_root.join(relative_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

fn assert_exact_ids(surface: &str, documented: &BTreeSet<&str>, expected: &BTreeSet<&str>) {
    let missing: Vec<_> = expected
        .iter()
        .copied()
        .filter(|id| !documented.contains(id))
        .collect();
    assert!(
        missing.is_empty(),
        "{surface} does not document expected checks: {missing:?}"
    );

    let unknown: Vec<_> = documented
        .iter()
        .copied()
        .filter(|id| !expected.contains(id))
        .collect();
    assert!(
        unknown.is_empty(),
        "{surface} documents checks outside its expected partition: {unknown:?}"
    );
}

fn assert_no_unknown_check_ids(path: &str, markdown: &str, catalog: &BTreeSet<&str>) {
    if let Some(token) = inline_code_tokens(markdown).into_iter().find(|token| {
        looks_like_check_id(token)
            && !catalog.contains(token)
            && !NON_CHECK_ID_LIKE_TOKENS.contains(token)
    }) {
        panic!("{path} names check-like id `{token}` that is not registered");
    }
}

fn registered_check_ids() -> BTreeSet<&'static str> {
    unique_check_ids(all_checks(), "registered")
}

fn registered_mechanical_check_ids() -> BTreeSet<&'static str> {
    unique_check_ids(mechanical_checks(), "mechanical")
}

fn unique_check_ids(
    checks: Vec<Box<dyn animsmith_core::Check>>,
    catalog_name: &str,
) -> BTreeSet<&'static str> {
    let ids: Vec<_> = checks.iter().map(|check| check.id()).collect();
    let unique: BTreeSet<_> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len(), "duplicate {catalog_name} check id");
    unique
}

fn readme_mechanical_check_table_ids(readme: &str) -> BTreeSet<&str> {
    check_table_ids_after(readme, "Mechanical checks")
}

fn readme_contract_aware_check_table_ids(readme: &str) -> BTreeSet<&str> {
    check_table_ids_after(readme, "Contract-aware checks")
}

fn check_table_ids_after<'a>(markdown: &'a str, marker: &str) -> BTreeSet<&'a str> {
    markdown_table_after(markdown, marker)
        .into_iter()
        .skip(2)
        .filter_map(|row| table_cell(row, 0))
        .flat_map(inline_code_tokens)
        .collect()
}

fn guide_symptom_table_ids(guide: &str) -> BTreeSet<&str> {
    markdown_table_after(guide, "From symptom to command")
        .into_iter()
        .skip(2)
        .filter_map(|row| table_cell(row, 1))
        .flat_map(inline_code_tokens)
        .collect()
}

fn guide_file_ready_check_ids<'a>(guide: &'a str, catalog: &BTreeSet<&str>) -> BTreeSet<&'a str> {
    inline_code_tokens(markdown_between(
        guide,
        "1. **File-ready**",
        "2. **Clip-ready**",
    ))
    .into_iter()
    .filter(|token| catalog.contains(token))
    .collect()
}

fn markdown_between<'a>(markdown: &'a str, start: &str, end: &str) -> &'a str {
    let start_offset = markdown
        .find(start)
        .unwrap_or_else(|| panic!("missing marker: {start}"));
    let rest = &markdown[start_offset..];
    let end_offset = rest
        .find(end)
        .unwrap_or_else(|| panic!("missing marker: {end}"));
    &rest[..end_offset]
}

fn markdown_table_after<'a>(markdown: &'a str, marker: &str) -> Vec<&'a str> {
    let mut lines = markdown.lines().skip_while(|line| !line.contains(marker));
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

#[test]
fn unknown_check_id_failure_names_the_file_and_id() {
    let catalog = BTreeSet::from(["known-check"]);
    let failure = std::panic::catch_unwind(|| {
        assert_no_unknown_check_ids("docs/example.md", "`stale-check`", &catalog);
    })
    .expect_err("a stale check id must fail the docs gate");
    let message = failure
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied())
        .expect("panic payload must be a string");

    assert!(message.contains("docs/example.md"), "{message}");
    assert!(message.contains("stale-check"), "{message}");
}
