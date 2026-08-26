use animsmith_core::{all_checks, mechanical_checks};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const NON_CHECK_ID_LIKE_TOKENS: &[&str] = &[
    "animsmith",
    "animsmith-core",
    "animsmith-engine",
    "animsmith-fbx",
    "animsmith-gltf",
    "animsmith-report",
    "bevy",
    "engine-addressability",
    "engine-track-support",
    "engine-unit-scale",
    "fix",
    "gltf-asset-loader",
    "humanoid",
    "lint",
    "measure",
    "mixamo",
    "transform",
    "ue-mannequin",
];
const PIPELINE_MATRIX_MARKER: &str = "the contract grows to cover them or the team accepts them:";

#[test]
fn docs_check_ids_match_the_registered_catalog() {
    let Some((readme, game_ready_clips, pipeline_scenarios)) = read_source_catalog_docs() else {
        // Published crates intentionally exclude repository-level docs.
        return;
    };

    assert_catalog_docs(&readme, &game_ready_clips, &pipeline_scenarios);
}

fn assert_catalog_docs(readme: &str, game_ready_clips: &str, pipeline_scenarios: &str) {
    let catalog = registered_check_ids();
    let mechanical = registered_mechanical_check_ids();
    let contract_aware: BTreeSet<_> = catalog.difference(&mechanical).copied().collect();

    assert_exact_ids(
        "README.md Mechanical checks table",
        &check_table_ids_after(readme, "Mechanical checks"),
        &mechanical,
    );
    assert_exact_ids(
        "README.md Contract-aware checks table",
        &check_table_ids_after(readme, "Contract-aware checks"),
        &contract_aware,
    );
    assert_exact_ids(
        "docs/game-ready-clips.md symptom table",
        &guide_symptom_table_ids(game_ready_clips),
        &catalog,
    );
    assert_exact_ids(
        "docs/game-ready-clips.md File-ready level",
        &guide_file_ready_check_ids(game_ready_clips),
        &mechanical,
    );

    for (path, markdown) in [
        ("README.md", readme),
        ("docs/game-ready-clips.md", game_ready_clips),
    ] {
        assert_no_unknown_check_ids(path, markdown, &catalog);
    }

    let pipeline_matrix =
        markdown_table_after(pipeline_scenarios, PIPELINE_MATRIX_MARKER).join("\n");
    assert_no_unknown_check_ids("docs/pipeline-scenarios.md", &pipeline_matrix, &catalog);
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

fn read_source_catalog_docs() -> Option<(String, String, String)> {
    let workspace_root = source_workspace_root(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    Some((
        read_workspace_doc(&workspace_root, "README.md"),
        read_workspace_doc(&workspace_root, "docs/game-ready-clips.md"),
        read_workspace_doc(&workspace_root, "docs/pipeline-scenarios.md"),
    ))
}

fn assert_exact_ids(surface: &str, documented: &BTreeSet<&str>, expected: &BTreeSet<&str>) {
    let missing: Vec<_> = expected
        .iter()
        .copied()
        .filter(|id| !documented.contains(id))
        .collect();
    let unexpected: Vec<_> = documented
        .iter()
        .copied()
        .filter(|id| !expected.contains(id))
        .collect();
    assert!(
        missing.is_empty() && unexpected.is_empty(),
        "{surface} check ids do not match; missing: {missing:?}; unexpected: {unexpected:?}"
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

fn guide_file_ready_check_ids(guide: &str) -> BTreeSet<&str> {
    inline_code_tokens(markdown_between(
        guide,
        "1. **File-ready**",
        "2. **Clip-ready**",
    ))
    .into_iter()
    .filter(|token| looks_like_check_id(token))
    .filter(|token| !NON_CHECK_ID_LIKE_TOKENS.contains(token))
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

fn panic_message(failure: Box<dyn std::any::Any + Send>) -> String {
    let message = failure
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied())
        .expect("panic payload must be a string");
    message.to_owned()
}

#[test]
fn partial_doc_scan_covers_every_matrix_row_without_requiring_the_complete_catalog() {
    let Some((readme, game_ready_clips, pipeline_scenarios)) = read_source_catalog_docs() else {
        return;
    };
    let catalog = registered_check_ids();
    let pipeline_matrix =
        markdown_table_after(&pipeline_scenarios, PIPELINE_MATRIX_MARKER).join("\n");
    let matrix_check_ids: BTreeSet<_> = inline_code_tokens(&pipeline_matrix)
        .into_iter()
        .filter(|token| catalog.contains(token))
        .collect();
    assert!(!matrix_check_ids.is_empty(), "matrix must name checks");
    assert!(
        matrix_check_ids.is_subset(&catalog) && matrix_check_ids.len() < catalog.len(),
        "the matrix must remain a partial catalog reference"
    );
    let partial_result = std::panic::catch_unwind(|| {
        assert_catalog_docs(&readme, &game_ready_clips, &pipeline_scenarios);
    });
    assert!(
        partial_result.is_ok(),
        "the partial matrix must not reproduce the complete catalog"
    );

    for documented in matrix_check_ids {
        let stale = format!("stale-{documented}");
        let stale_matrix =
            pipeline_matrix.replace(&format!("`{documented}`"), &format!("`{stale}`"));
        assert_ne!(stale_matrix, pipeline_matrix, "mutation must apply");
        let stale_pipeline = format!("{PIPELINE_MATRIX_MARKER}\n\n{stale_matrix}");

        let failure = std::panic::catch_unwind(|| {
            assert_catalog_docs(&readme, &game_ready_clips, &stale_pipeline);
        })
        .expect_err("a stale partial-doc check id must fail the docs gate");
        let message = panic_message(failure);

        assert!(message.contains("docs/pipeline-scenarios.md"), "{message}");
        assert!(message.contains(&stale), "{message}");
    }
}

#[test]
fn both_readme_partition_directions_fail_the_complete_docs_gate() {
    let Some((readme, game_ready_clips, pipeline_scenarios)) = read_source_catalog_docs() else {
        return;
    };
    for (documented, misplaced, surface, offending) in [
        (
            "| `scale-keys` | warning",
            "| `fps` | warning",
            "README.md Mechanical checks table",
            "fps",
        ),
        (
            "| `fps` | warning",
            "| `scale-keys` | warning",
            "README.md Contract-aware checks table",
            "scale-keys",
        ),
    ] {
        let misplaced_readme = readme.replacen(documented, misplaced, 1);
        assert_ne!(misplaced_readme, readme, "mutation must apply");

        let failure = std::panic::catch_unwind(|| {
            assert_catalog_docs(&misplaced_readme, &game_ready_clips, &pipeline_scenarios);
        })
        .expect_err("putting an id in the wrong README partition must fail");
        let message = panic_message(failure);

        assert!(message.contains(surface), "{message}");
        assert!(message.contains(offending), "{message}");
    }

    let swapped_readme = readme
        .replacen(
            "| `scale-keys` | warning",
            "| `partition-swap-placeholder` | warning",
            1,
        )
        .replacen("| `fps` | warning", "| `scale-keys` | warning", 1)
        .replacen(
            "| `partition-swap-placeholder` | warning",
            "| `fps` | warning",
            1,
        );
    assert_ne!(swapped_readme, readme, "swap mutation must apply");
    let failure = std::panic::catch_unwind(|| {
        assert_catalog_docs(&swapped_readme, &game_ready_clips, &pipeline_scenarios);
    })
    .expect_err("swapping ids across README partitions must fail");
    let message = panic_message(failure);
    assert!(
        message.contains("README.md Mechanical checks table"),
        "{message}"
    );
    assert!(message.contains("fps"), "{message}");
}

#[test]
fn file_ready_partition_mutation_fails_the_complete_docs_gate() {
    let Some((readme, game_ready_clips, pipeline_scenarios)) = read_source_catalog_docs() else {
        return;
    };
    let file_ready = markdown_between(&game_ready_clips, "1. **File-ready**", "2. **Clip-ready**");
    for (replacement, offending) in [("`fps`", "fps"), ("", "constant-track")] {
        let misplaced_file_ready = file_ready.replacen("`constant-track`", replacement, 1);
        assert_ne!(misplaced_file_ready, file_ready, "mutation must apply");
        let misplaced_guide = game_ready_clips.replacen(file_ready, &misplaced_file_ready, 1);

        let failure = std::panic::catch_unwind(|| {
            assert_catalog_docs(&readme, &misplaced_guide, &pipeline_scenarios);
        })
        .expect_err("File-ready partition drift must fail");
        let message = panic_message(failure);

        assert!(
            message.contains("docs/game-ready-clips.md File-ready level"),
            "{message}"
        );
        assert!(message.contains(offending), "{message}");
    }
}
