use animsmith_core::{all_checks, mechanical_checks};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::{BTreeMap, BTreeSet};
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
    "engine-root-motion",
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
const REQUIRED_DETAIL_FIELDS: &[&str] = &[
    "Default findings:",
    "Prerequisites and applicability:",
    "Config, defaults, and units:",
    "Remediation and boundary:",
];

#[test]
fn docs_check_ids_match_the_registered_catalog() {
    let Some((readme, game_ready_clips, pipeline_scenarios, built_in_checks)) =
        read_source_catalog_docs()
    else {
        // Published crates intentionally exclude repository-level docs.
        return;
    };

    assert_catalog_docs(
        &readme,
        &game_ready_clips,
        &pipeline_scenarios,
        &built_in_checks,
    );
}

fn assert_catalog_docs(
    readme: &str,
    game_ready_clips: &str,
    pipeline_scenarios: &str,
    built_in_checks: &str,
) {
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
    assert_built_in_check_details(built_in_checks, &catalog);
    assert_built_in_check_inventory(built_in_checks, &catalog);
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

fn read_source_catalog_docs() -> Option<(String, String, String, String)> {
    let workspace_root = source_workspace_root(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    Some((
        read_workspace_doc(&workspace_root, "README.md"),
        read_workspace_doc(&workspace_root, "docs/game-ready-clips.md"),
        read_workspace_doc(&workspace_root, "docs/pipeline-scenarios.md"),
        read_workspace_doc(&workspace_root, "docs/built-in-checks.md"),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckReferenceInventoryRow {
    id: &'static str,
    default_findings: &'static [&'static str],
    source: &'static str,
    severity_tokens: &'static [&'static str],
    config_access: &'static [(&'static str, &'static str)],
}

fn built_in_check_inventory_rows() -> Vec<CheckReferenceInventoryRow> {
    macro_rules! finding_name {
        (Error) => {
            "error"
        };
        (Warning) => {
            "warning"
        };
        (Note) => {
            "note"
        };
        (Off) => {
            "off"
        };
    }
    macro_rules! severity_token {
        (Error) => {
            "Severity::Error"
        };
        (Warning) => {
            "Severity::Warning"
        };
        (Note) => {
            "Severity::Note"
        };
        (Off) => {
            ""
        };
    }
    macro_rules! row {
        ($id:literal, $source:literal, [$($severity:ident),+], {$($key:literal => $access:literal),* $(,)?}) => {
            CheckReferenceInventoryRow {
                id: $id,
                default_findings: &[$(finding_name!($severity)),+],
                source: $source,
                severity_tokens: &[$(severity_token!($severity)),+],
                config_access: &[$(($key, $access)),*],
            }
        };
    }
    vec![
        row!("nan", "crates/animsmith-core/src/checks/nan.rs", [Error], {"severity" => ""}),
        row!("time-monotonic", "crates/animsmith-core/src/checks/time_monotonic.rs", [Error, Note], {"severity" => ""}),
        row!("quat-norm", "crates/animsmith-core/src/checks/quat_norm.rs", [Error], {"severity" => ""}),
        row!("quat-flip", "crates/animsmith-core/src/checks/quat_flip.rs", [Warning], {"severity" => ""}),
        row!("duration-sanity", "crates/animsmith-core/src/checks/duration_sanity.rs", [Error, Warning], {"severity" => "", "clips.<name>.duration_s.value" => "duration_s", "clips.<name>.duration_s.tolerance" => "tolerance"}),
        row!("scale-keys", "crates/animsmith-core/src/checks/scale_keys.rs", [Warning], {"severity" => ""}),
        row!("non-uniform-scale", "crates/animsmith-core/src/checks/non_uniform_scale.rs", [Warning], {"severity" => ""}),
        row!("constant-nonunit-scale", "crates/animsmith-core/src/checks/constant_nonunit_scale.rs", [Off, Note], {"severity" => ""}),
        row!("constant-track", "crates/animsmith-core/src/checks/constant_track.rs", [Note], {"severity" => ""}),
        row!("required-bones", "crates/animsmith-core/src/checks/required_bones.rs", [Error], {"severity" => "", "rig.required_bones" => "required_bones"}),
        row!("rest-world-scale", "crates/animsmith-core/src/checks/rest_world_scale.rs", [Warning], {"severity" => "", "runtime_nodes.selectors" => "runtime_node_selectors", "checks.rest-world-scale.node_selectors" => "node_selectors", "expected_uniform_scale" => "expected_uniform_scale", "uniform_scale_tolerance" => "uniform_scale_tolerance"}),
        row!("missing-bones", "crates/animsmith-core/src/checks/missing_bones.rs", [Error], {"severity" => "", "clips.<name>.animates_bones" => "animates_bones"}),
        row!("frozen-bone", "crates/animsmith-core/src/checks/frozen_bone.rs", [Error], {"severity" => "", "min_rotation_deg" => "min_rotation_deg", "clips.<name>.animates_bones" => "animates_bones"}),
        row!("duplicate-loop-endpoint", "crates/animsmith-core/src/checks/duplicate_loop_endpoint.rs", [Warning], {"severity" => "", "clips.<name>.loop" => "looping"}),
        row!("loop-closure", "crates/animsmith-core/src/checks/loop_closure.rs", [Error], {"severity" => "", "max_position_delta_m" => "max_position_delta_m", "max_rotation_delta_deg" => "max_rotation_delta_deg", "clips.<name>.loop" => "looping", "clips.<name>.max_loop_position_delta_m" => "max_loop_position_delta_m", "clips.<name>.max_loop_rotation_delta_deg" => "max_loop_rotation_delta_deg"}),
        row!("loop-seam", "crates/animsmith-core/src/checks/loop_seam.rs", [Error], {"severity" => "", "max_ratio" => "max_ratio", "min_stride_step_m" => "loop_seam_min_stride_step_m", "clips.<name>.loop" => "looping"}),
        row!("loop-seam-vel", "crates/animsmith-core/src/checks/loop_seam_vel.rs", [Error], {"severity" => "", "max_velocity_delta_mps" => "max_velocity_delta_mps", "clips.<name>.loop" => "looping", "clips.<name>.max_loop_velocity_delta_mps" => "max_loop_velocity_delta_mps"}),
        row!("loop-seam-rot", "crates/animsmith-core/src/checks/loop_seam_rot.rs", [Error], {"severity" => "", "max_angular_velocity_delta_degps" => "max_angular_velocity_delta_degps", "clips.<name>.loop" => "looping", "clips.<name>.max_loop_angular_velocity_delta_degps" => "max_loop_angular_velocity_delta_degps"}),
        row!("root-motion-speed", "crates/animsmith-core/src/checks/root_motion_speed.rs", [Error], {"severity" => "", "clips.<name>.speed_mps.value" => "speed_mps", "clips.<name>.speed_mps.tolerance" => "tolerance", "clips.<name>.movement_owner_xz" => "movement_owner_xz", "clips.<name>.in_place" => "movement_owner_xz"}),
        row!("gait-group", "crates/animsmith-core/src/checks/gait_group.rs", [Error], {"severity" => "", "gait_groups.<name>.clips" => "gait_groups", "gait_groups.<name>.max_gait_phase_spread" => "max_gait_phase_spread", "gait_groups.<name>.min_lr_amplitude_m" => "min_lr_amplitude_m"}),
        row!("sync-group", "crates/animsmith-core/src/checks/sync_group.rs", [Error], {"severity" => "", "sync_groups.<name>.clips" => "sync_groups", "sync_groups.<name>.max_duration_delta_s" => "max_duration_delta_s", "sync_groups.<name>.max_frame_count_delta" => "max_frame_count_delta", "sync_groups.<name>.max_fps_delta" => "max_fps_delta"}),
        row!("time-complement", "crates/animsmith-core/src/checks/time_complement.rs", [Warning], {"severity" => "", "sync_groups.<name>.clips" => "sync_groups", "sync_groups.<name>.time_complement.min_reflected_time_advantage" => "min_reflected_time_advantage", "sync_groups.<name>.time_complement.min_lr_amplitude_m" => "min_lr_amplitude_m"}),
        row!("in-place", "crates/animsmith-core/src/checks/in_place.rs", [Error], {"severity" => "", "clips.<name>.movement_owner_xz" => "movement_owner_xz", "clips.<name>.in_place" => "movement_owner_xz"}),
        row!("fps", "crates/animsmith-core/src/checks/fps.rs", [Warning], {"severity" => "", "clips.<name>.fps" => "fps"}),
        row!("bind-pose", "crates/animsmith-core/src/checks/bind_pose.rs", [Warning], {"severity" => "", "max_mean_rest_delta_deg" => "max_mean_rest_delta_deg"}),
        row!("foot-slide", "crates/animsmith-core/src/checks/foot_slide.rs", [Warning], {"severity" => "", "contact_height_m" => "contact_height_m", "max_slide_mps" => "max_slide_mps"}),
    ]
}

fn split_markdown_table_row(row: &str) -> Vec<String> {
    row.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect()
}

fn markdown_link_label(cell: &str) -> &str {
    let Some(stripped) = cell.strip_prefix('[') else {
        return cell;
    };
    let Some((label, rest)) = stripped.split_once(']') else {
        return cell;
    };
    if rest.starts_with('(') && rest.ends_with(')') {
        label
    } else {
        cell
    }
}

fn inventory_table(markdown: &str) -> MarkdownTable {
    let rows = markdown_table_after(markdown, "## Inventory");
    assert!(
        rows.len() >= 2,
        "docs/built-in-checks.md must contain the inventory table"
    );
    let headers = split_markdown_table_row(rows[0]);
    assert_eq!(
        headers,
        vec![
            "id".to_owned(),
            "class".to_owned(),
            "default findings".to_owned(),
            "declarations or prerequisites".to_owned(),
            "config keys".to_owned(),
            "tooling".to_owned(),
        ],
        "docs/built-in-checks.md inventory headers drifted"
    );
    MarkdownTable {
        headers,
        rows: rows
            .into_iter()
            .skip(2)
            .map(split_markdown_table_row)
            .collect(),
    }
}

fn cell_tokens(cell: &str) -> BTreeSet<&str> {
    cell.split(',')
        .map(|token| token.trim().trim_matches('`'))
        .filter(|token| !token.is_empty())
        .collect()
}

fn assert_built_in_check_inventory(markdown: &str, catalog: &BTreeSet<&str>) {
    let table = inventory_table(markdown);
    let row_by_id = table
        .rows
        .iter()
        .map(|row| {
            assert_eq!(
                row.len(),
                6,
                "docs/built-in-checks.md inventory rows must keep six columns"
            );
            (markdown_link_label(&row[0]).to_owned(), row)
        })
        .collect::<BTreeMap<_, _>>();

    let documented_ids = row_by_id
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_exact_ids(
        "docs/built-in-checks.md inventory",
        &documented_ids,
        catalog,
    );

    let expected_rows = built_in_check_inventory_rows();
    let expected_ids = expected_rows
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        expected_ids, *catalog,
        "inventory expectations must cover the exact registered catalog"
    );

    for expected in expected_rows {
        let row = row_by_id
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing inventory row for {}", expected.id));
        assert_eq!(
            cell_tokens(&row[2]),
            expected.default_findings.iter().copied().collect(),
            "docs/built-in-checks.md default findings drifted for {}",
            expected.id
        );
        let documented_config = cell_tokens(&row[4]);
        let expected_config = expected
            .config_access
            .iter()
            .map(|(key, _)| *key)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            documented_config, expected_config,
            "docs/built-in-checks.md config keys drifted for {}",
            expected.id
        );

        let workspace_root = source_workspace_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("source docs imply a source checkout");
        let source = read_workspace_doc(&workspace_root, expected.source);
        assert_source_tokens(expected.id, &source, expected.severity_tokens);
        assert_default_enablement(
            expected.id,
            &source,
            !expected.default_findings.contains(&"off"),
        );
        for (documented_key, access_token) in expected.config_access {
            if !access_token.is_empty() {
                assert_source_tokens(expected.id, &source, &[*access_token]);
            }
            assert!(
                documented_config.contains(documented_key),
                "missing documented config key {documented_key} for {}",
                expected.id
            );
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct MarkdownSection {
    heading: String,
    body: String,
}

fn built_in_check_sections(markdown: &str) -> Vec<MarkdownSection> {
    let events: Vec<_> = Parser::new_ext(markdown, Options::all()).collect();
    let mut sections = Vec::new();

    for (index, event) in events.iter().enumerate() {
        if !matches!(
            event,
            Event::Start(Tag::Heading {
                level: HeadingLevel::H3,
                ..
            })
        ) {
            continue;
        }

        let end = (index + 1..events.len())
            .find(|candidate| matches!(events[*candidate], Event::End(TagEnd::Heading(_))))
            .expect("every Markdown heading must have an end event");
        let heading = markdown_event_text(&events[index + 1..end]);
        let body_end = (end + 1..events.len())
            .find(|candidate| markdown_heading_at_or_above_h3(&events[*candidate]))
            .unwrap_or(events.len());
        let body = markdown_event_text(&events[end + 1..body_end]);
        sections.push(MarkdownSection { heading, body });
    }

    sections
}

fn markdown_heading_at_or_above_h3(event: &Event<'_>) -> bool {
    matches!(
        event,
        Event::Start(Tag::Heading {
            level: HeadingLevel::H1 | HeadingLevel::H2 | HeadingLevel::H3,
            ..
        })
    )
}

fn markdown_event_text(events: &[Event<'_>]) -> String {
    let mut text = String::new();
    for event in events {
        match event {
            Event::Text(value) | Event::Code(value) | Event::Html(value) => text.push_str(value),
            Event::SoftBreak | Event::HardBreak => text.push('\n'),
            _ => {}
        }
    }
    text
}

fn assert_built_in_check_details(markdown: &str, catalog: &BTreeSet<&str>) {
    let sections = built_in_check_sections(markdown);
    let mut seen = BTreeSet::new();
    for section in &sections {
        if catalog.contains(section.heading.as_str()) {
            assert!(
                seen.insert(section.heading.as_str()),
                "docs/built-in-checks.md has duplicate detailed section for `{}`",
                section.heading
            );
            for field in REQUIRED_DETAIL_FIELDS {
                assert!(
                    section.body.contains(field),
                    "docs/built-in-checks.md detailed section for `{}` is missing `{field}`",
                    section.heading
                );
            }
        }
    }
    assert_exact_ids("docs/built-in-checks.md detailed sections", &seen, catalog);
}

fn assert_source_tokens(id: &str, source: &str, tokens: &[&str]) {
    for token in tokens {
        if token.is_empty() {
            continue;
        }
        assert!(
            source.contains(token),
            "implementation source for {id} does not contain expected token {token:?}"
        );
    }
}

fn assert_default_enablement(id: &str, source: &str, expected_enabled: bool) {
    let compact: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let signature = "fnenabled_by_default(&self)->bool{";
    let Some(start) = compact.find(signature) else {
        assert!(
            expected_enabled,
            "implementation source for opt-in check {id} must override enabled_by_default"
        );
        return;
    };
    let body = &compact[start + signature.len()..];
    let actual = body
        .split('}')
        .next()
        .expect("enabled_by_default override must have a body");
    let expected = if expected_enabled { "true" } else { "false" };
    assert_eq!(
        actual, expected,
        "implementation source for {id} disagrees with documented default enablement"
    );
}

#[test]
fn source_token_helper_rejects_a_nonexistent_token() {
    let failure = std::panic::catch_unwind(|| {
        assert_source_tokens(
            "fixture",
            "Finding::new(Severity::Warning)",
            &["not_a_real_token"],
        );
    })
    .expect_err("a nonexistent implementation token must be rejected");
    let message = panic_message(failure);
    assert!(message.contains("not_a_real_token"), "{message}");
}

#[test]
fn default_enablement_helper_rejects_a_mutated_opt_in_authority() {
    let source = "fn enabled_by_default(&self) -> bool { false }";
    let mutated = source.replacen("false", "true", 1);
    let failure = std::panic::catch_unwind(|| {
        assert_default_enablement("constant-nonunit-scale", &mutated, false);
    })
    .expect_err("a default-on mutation must be rejected for an opt-in check");
    let message = panic_message(failure);
    assert!(message.contains("constant-nonunit-scale"), "{message}");
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
    let Some((readme, game_ready_clips, pipeline_scenarios, built_in_checks)) =
        read_source_catalog_docs()
    else {
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
        assert_catalog_docs(
            &readme,
            &game_ready_clips,
            &pipeline_scenarios,
            &built_in_checks,
        );
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
            assert_catalog_docs(
                &readme,
                &game_ready_clips,
                &stale_pipeline,
                &built_in_checks,
            );
        })
        .expect_err("a stale partial-doc check id must fail the docs gate");
        let message = panic_message(failure);

        assert!(message.contains("docs/pipeline-scenarios.md"), "{message}");
        assert!(message.contains(&stale), "{message}");
    }
}

#[test]
fn both_readme_partition_directions_fail_the_complete_docs_gate() {
    let Some((readme, game_ready_clips, pipeline_scenarios, built_in_checks)) =
        read_source_catalog_docs()
    else {
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
            assert_catalog_docs(
                &misplaced_readme,
                &game_ready_clips,
                &pipeline_scenarios,
                &built_in_checks,
            );
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
        assert_catalog_docs(
            &swapped_readme,
            &game_ready_clips,
            &pipeline_scenarios,
            &built_in_checks,
        );
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
    let Some((readme, game_ready_clips, pipeline_scenarios, built_in_checks)) =
        read_source_catalog_docs()
    else {
        return;
    };
    let file_ready = markdown_between(&game_ready_clips, "1. **File-ready**", "2. **Clip-ready**");
    for (replacement, offending) in [("`fps`", "fps"), ("", "constant-track")] {
        let misplaced_file_ready = file_ready.replacen("`constant-track`", replacement, 1);
        assert_ne!(misplaced_file_ready, file_ready, "mutation must apply");
        let misplaced_guide = game_ready_clips.replacen(file_ready, &misplaced_file_ready, 1);

        let failure = std::panic::catch_unwind(|| {
            assert_catalog_docs(
                &readme,
                &misplaced_guide,
                &pipeline_scenarios,
                &built_in_checks,
            );
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

#[test]
fn stale_or_missing_built_in_check_inventory_rows_fail_the_docs_gate() {
    let Some((readme, game_ready_clips, pipeline_scenarios, built_in_checks)) =
        read_source_catalog_docs()
    else {
        return;
    };

    let stale = built_in_checks.replacen("`max_slide_mps`", "`max_slide_speed_mps`", 1);
    assert_ne!(stale, built_in_checks, "mutation must apply");
    let failure = std::panic::catch_unwind(|| {
        assert_catalog_docs(&readme, &game_ready_clips, &pipeline_scenarios, &stale);
    })
    .expect_err("stale inventory config keys must fail");
    let message = panic_message(failure);
    assert!(message.contains("foot-slide"), "{message}");

    let missing = built_in_checks
        .lines()
        .filter(|line| !line.starts_with("| [bind-pose](#bind-pose) |"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(missing, built_in_checks, "mutation must apply");
    let failure = std::panic::catch_unwind(|| {
        assert_catalog_docs(&readme, &game_ready_clips, &pipeline_scenarios, &missing);
    })
    .expect_err("missing inventory rows must fail");
    let message = panic_message(failure);
    assert!(
        message.contains("docs/built-in-checks.md inventory"),
        "{message}"
    );
    assert!(message.contains("bind-pose"), "{message}");
}

#[test]
fn missing_detailed_check_section_fails_even_when_inventory_is_intact() {
    let Some((readme, game_ready_clips, pipeline_scenarios, built_in_checks)) =
        read_source_catalog_docs()
    else {
        return;
    };
    let missing = built_in_checks.replacen("### `bind-pose`\n", "", 1);
    assert_ne!(
        missing, built_in_checks,
        "mutation must remove a detailed section"
    );
    assert!(
        missing.contains("| [bind-pose](#bind-pose) |"),
        "the mutation must leave the inventory row intact"
    );

    let failure = std::panic::catch_unwind(|| {
        assert_catalog_docs(&readme, &game_ready_clips, &pipeline_scenarios, &missing);
    })
    .expect_err("a missing detailed section must fail the docs gate");
    let message = panic_message(failure);
    assert!(message.contains("detailed sections"), "{message}");
    assert!(message.contains("bind-pose"), "{message}");
}
