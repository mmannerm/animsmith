//! Contract tests for the mdBook staging boundary.  The book generator may
//! parse the canonical index mechanically, but staged Markdown is validated
//! with pulldown-cmark so the check covers what a renderer actually sees.

use animsmith_testkit::{docs_html, docs_markdown};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Generated group chapters live outside the canonical tree, so navigation
/// can distinguish them from the rows they collect.
const GENERATED_GROUP_DIR: &str = "_generated/groups/";

/// The canonical Category cell separates a part title from its group with a
/// spaced single right-pointing angle quotation mark.
const GROUP_SEPARATOR: &str = " \u{203a} ";

/// The tracked theme bridge that pins an embedded report to the book theme.
const THEME_SCRIPT: &str = "animsmith.js";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES
}

fn links(markdown: &str) -> Vec<String> {
    Parser::new_ext(markdown, options())
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.into_string()),
            _ => None,
        })
        .collect()
}

/// Every generated chapter as `(part title, label, destination, list depth)`.
/// Parts are mdBook part headings; depth 1 is a top-level chapter, depth 2 a
/// group member or a report nested under an ungrouped index, and so on.
fn summary_chapters(markdown: &str) -> Vec<(String, String, String, usize)> {
    let mut part = String::new();
    let mut heading = None;
    let mut list_depth = 0usize;
    let mut active: Option<(String, String)> = None;
    let mut chapters = Vec::new();
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(text) = heading.take() {
                    part = text;
                }
            }
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth -= 1,
            Event::Start(Tag::Link { dest_url, .. }) => {
                active = Some((String::new(), dest_url.into_string()))
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((label, _)) = active.as_mut() {
                    label.push_str(&text);
                } else if let Some(heading) = heading.as_mut() {
                    heading.push_str(&text);
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some((label, destination)) = active.take() {
                    chapters.push((part.clone(), label, destination, list_depth));
                }
            }
            _ => {}
        }
    }
    chapters
}

/// The first link of every top-level list item, which is how a generated group
/// page presents one canonical member row before its description.
fn first_item_links(markdown: &str) -> Vec<(String, String)> {
    let mut open = false;
    let mut active: Option<(String, String)> = None;
    let mut links = Vec::new();
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Item) => open = true,
            Event::End(TagEnd::Item) => open = false,
            Event::Start(Tag::Link { dest_url, .. }) if open => {
                active = Some((String::new(), dest_url.into_string()))
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((label, _)) = active.as_mut() {
                    label.push_str(&text);
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some(link) = active.take() {
                    links.push(link);
                    open = false;
                }
            }
            _ => {}
        }
    }
    links
}

fn rendered_links(markdown: &str) -> Vec<(String, String)> {
    let mut rendered = Vec::new();
    let mut active = None;
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                active = Some((String::new(), dest_url.into_string()))
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((label, _)) = active.as_mut() {
                    label.push_str(&text);
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some(link) = active.take() {
                    rendered.push(link);
                }
            }
            _ => {}
        }
    }
    rendered
}

fn rendered_headings(markdown: &str) -> Vec<String> {
    let mut rendered = Vec::new();
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
                    rendered.push(heading);
                }
            }
            _ => {}
        }
    }
    rendered
}

/// Keep staged-page anchors identical to the parser-backed repository-link
/// gate in `docs_links.rs`: GitHub-style slugging and duplicate suffixes, not
/// a line-oriented heading regex.
fn github_slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|character| match character {
            ' ' => Some('-'),
            '-' | '_' => Some(character),
            character if character.is_alphanumeric() => Some(character),
            _ => None,
        })
        .collect()
}

fn heading_anchors(markdown: &str) -> BTreeSet<String> {
    let mut anchors = BTreeSet::new();
    let mut counts = BTreeMap::new();
    let mut heading = None;
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(text) = heading.take() {
                    let base = github_slug(&text);
                    let mut candidate = base.clone();
                    while anchors.contains(&candidate) {
                        let count = counts.entry(base.clone()).or_insert(0usize);
                        *count += 1;
                        candidate = format!("{base}-{count}");
                    }
                    anchors.insert(candidate);
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(heading) = heading.as_mut() {
                    heading.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(heading) = heading.as_mut() {
                    heading.push(' ');
                }
            }
            _ => {}
        }
    }
    anchors
}

fn staged_local_target(staged: &Path, page: &Path, local: &str) -> Result<PathBuf, &'static str> {
    if (local.starts_with('/') || local.starts_with('\\'))
        || local.contains('\\')
        || local
            .as_bytes()
            .get(1)
            .is_some_and(|character| *character == b':')
            && local.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err("rooted or non-URL local path");
    }
    let page_relative = page
        .strip_prefix(staged)
        .map_err(|_| "page is outside staged source")?;
    let candidate = if local.is_empty() {
        page_relative.to_path_buf()
    } else {
        page_relative
            .parent()
            .ok_or("staged page has no parent")?
            .join(local)
    };
    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(component) => normalized.push(component),
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err("local path escapes staged source");
                }
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err("rooted local path");
            }
        }
    }
    Ok(staged.join(normalized))
}

fn validate_staged_links(staged: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    let mut anchors = BTreeMap::new();
    for page in markdown_files(staged) {
        let text = std::fs::read_to_string(&page).expect("reads staged Markdown");
        for destination in links(&text) {
            if destination.contains("://") || destination.starts_with("mailto:") {
                continue;
            }
            let (local, fragment) = destination
                .split_once('#')
                .map_or((destination.as_str(), None), |(local, fragment)| {
                    (local, Some(fragment))
                });
            let target = match staged_local_target(staged, &page, local) {
                Ok(target) => target,
                Err(reason) => {
                    errors.push(format!(
                        "{} renders a local link outside staged source ({reason}): {destination}",
                        page.strip_prefix(staged).expect("page is staged").display()
                    ));
                    continue;
                }
            };
            if !target.exists() {
                errors.push(format!(
                    "{} renders a link to missing staged target {destination}",
                    page.strip_prefix(staged).expect("page is staged").display()
                ));
                continue;
            }
            if let Some(fragment) = fragment
                && target.extension().and_then(|extension| extension.to_str()) == Some("md")
            {
                let target_anchors = anchors.entry(target.clone()).or_insert_with(|| {
                    heading_anchors(&std::fs::read_to_string(&target).expect("reads anchor target"))
                });
                if !target_anchors.contains(fragment) {
                    errors.push(format!(
                        "{} renders an unresolved staged anchor {destination}",
                        page.strip_prefix(staged).expect("page is staged").display()
                    ));
                }
            }
        }
    }
    errors
}

fn markdown_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in walkdir(root) {
        if entry.extension().and_then(|extension| extension.to_str()) == Some("md") {
            files.push(entry);
        }
    }
    files.sort();
    files
}

fn walkdir(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(root).expect("lists staged directory") {
        let path = entry.expect("directory entry").path();
        let metadata = std::fs::symlink_metadata(&path).expect("staged metadata");
        assert!(
            !metadata.file_type().is_symlink(),
            "staging must not contain symlinks: {path:?}"
        );
        if metadata.is_dir() {
            paths.extend(walkdir(&path));
        } else {
            paths.push(path);
        }
    }
    paths
}

/// Staging refuses a checkout with no tracked stylesheet, so every fixture
/// carries the one asset `book.toml` wires.
fn write_fixture_theme(root: &Path) {
    std::fs::create_dir_all(root.join("docs/site")).expect("creates fixture theme directory");
    std::fs::write(root.join("docs/site/animsmith.css"), "/* fixture */\n")
        .expect("writes fixture stylesheet");
}

fn write_book_fixture(root: &Path, marker: &str, mdbook_pin: &str) {
    std::fs::create_dir_all(root.join("docs")).expect("creates fixture docs directory");
    std::fs::write(root.join(".mdbook-version"), format!("{mdbook_pin}\n"))
        .expect("writes mdBook pin");
    std::fs::write(root.join("README.md"), format!("# {marker}\n")).expect("writes root page");
    std::fs::write(
        root.join("docs/README.md"),
        format!("# {marker}\n\n| Document | Use it to… | Category |\n|---|---|---|\n| [Guide](guide.md) | Fixture guide. | Guides |\n"),
    )
    .expect("writes canonical fixture index");
    std::fs::write(root.join("docs/guide.md"), format!("# {marker} guide\n"))
        .expect("writes fixture guide");
    write_fixture_theme(root);
    assert!(
        Command::new("git")
            .args(["init", "--quiet", root.to_str().unwrap()])
            .status()
            .expect("initializes fixture checkout")
            .success()
    );
    assert!(
        Command::new("git")
            .args(["-C", root.to_str().unwrap(), "add", "."])
            .status()
            .expect("tracks fixture checkout")
            .success()
    );
}

fn write_external_index_fixture(root: &Path, label: &str, destination: &str) {
    std::fs::create_dir_all(root.join("docs")).expect("creates fixture docs directory");
    std::fs::write(root.join(".mdbook-version"), "0.4.52\n").expect("writes mdBook pin");
    std::fs::write(root.join("README.md"), "# fixture\n").expect("writes root page");
    std::fs::write(
        root.join("docs/README.md"),
        format!(
            "# Documentation\n\n| Document | Use it to… | Category |\n|---|---|---|\n| [{label}]({destination}) | External fixture. | Reference |\n"
        ),
    )
    .expect("writes external fixture index");
    write_fixture_theme(root);
    assert!(
        Command::new("git")
            .args(["init", "--quiet", root.to_str().unwrap()])
            .status()
            .expect("initializes fixture checkout")
            .success()
    );
    assert!(
        Command::new("git")
            .args(["-C", root.to_str().unwrap(), "add", "."])
            .status()
            .expect("tracks fixture checkout")
            .success()
    );
}

fn write_fixture_builder(path: &Path, builder: &str) {
    std::fs::create_dir_all(
        path.parent()
            .expect("fixture builder has a parent directory"),
    )
    .expect("creates fixture builder directory");
    std::fs::write(
        path,
        format!(
            r#"import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--source", type=Path, required=True)
parser.add_argument("--stage", type=Path, required=True)
parser.add_argument("--site-url")
parser.add_argument("--source-ref", required=True)
parser.add_argument("--mdbook", type=Path, required=True)
parser.add_argument("--build", action="store_true")
args = parser.parse_args()
(args.stage / "book").mkdir(parents=True)
marker = (args.source / "README.md").read_text(encoding="utf-8")
pin = (args.source / ".mdbook-version").read_text(encoding="utf-8")
(args.stage / "book" / "index.html").write_text(
    f"{{marker}}builder={builder}\nmdbook={{args.mdbook.name}}\npin={{pin}}source-ref={{args.source_ref}}\n",
    encoding="utf-8",
)
"#
        ),
    )
    .expect("writes deterministic fixture builder");
}

fn git(root: &Path, arguments: &[&str]) {
    assert!(
        Command::new("git")
            .args(["-C", root.to_str().expect("fixture path is UTF-8")])
            .args(arguments)
            .status()
            .expect("runs git for fixture")
            .success(),
        "git {:?} succeeds",
        arguments
    );
}

fn canonical_index_rows(markdown: &str) -> Vec<(String, String, String)> {
    let mut in_table = false;
    let mut in_head = false;
    let mut first_header = None;
    let mut header_cell = 0usize;
    let mut is_index = false;
    let mut cell = 0usize;
    let mut row_link = None;
    let mut row_label = String::new();
    let mut row_category = String::new();
    let mut rows = Vec::new();

    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                is_index = false;
                first_header = None;
                header_cell = 0;
            }
            Event::Start(Tag::TableHead) if in_table => in_head = true,
            Event::Start(Tag::TableCell) if in_head => {
                header_cell += 1;
                if header_cell == 1 {
                    first_header = Some(String::new());
                }
            }
            Event::Text(text) if in_head && header_cell == 1 => {
                first_header
                    .as_mut()
                    .expect("header is present")
                    .push_str(&text);
            }
            Event::End(TagEnd::TableHead) if in_table => {
                in_head = false;
                is_index = first_header.as_deref() == Some("Document");
            }
            Event::Start(Tag::TableRow) if is_index => {
                cell = 0;
                row_link = None;
                row_label.clear();
                row_category.clear();
            }
            Event::Start(Tag::TableCell) if is_index && !in_head => cell += 1,
            Event::Start(Tag::Link { dest_url, .. }) if is_index && !in_head && cell == 1 => {
                row_link = Some(dest_url.into_string());
            }
            Event::Text(text) | Event::Code(text) if is_index && !in_head && cell == 1 => {
                row_label.push_str(&text);
            }
            Event::Text(text) if is_index && !in_head && cell == 3 => {
                row_category.push_str(&text);
            }
            Event::End(TagEnd::TableRow) if is_index => rows.push((
                row_category.trim().to_owned(),
                row_label.trim().to_owned(),
                row_link.take().expect("index Document cell has a link"),
            )),
            Event::End(TagEnd::Table) if in_table => break,
            _ => {}
        }
    }
    rows
}

fn canonical_report_pair_links(markdown: &str) -> Vec<String> {
    let mut in_table = false;
    let mut in_head = false;
    let mut is_report_index = false;
    let mut first_header = None;
    let mut cell = 0usize;
    let mut links = Vec::new();
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                in_head = true;
                is_report_index = false;
                first_header = Some(String::new());
                cell = 0;
            }
            Event::Start(Tag::TableCell) if in_table => cell += 1,
            Event::Text(text) if in_table && in_head && cell == 1 => {
                first_header
                    .as_mut()
                    .expect("header is present")
                    .push_str(&text);
            }
            Event::End(TagEnd::TableHead) if in_table => {
                in_head = false;
                is_report_index = first_header.as_deref() == Some("Technical report");
            }
            Event::Start(Tag::TableRow) if is_report_index => cell = 0,
            Event::Start(Tag::Link { dest_url, .. })
                if is_report_index && !in_head && matches!(cell, 1 | 2) =>
            {
                links.push(dest_url.into_string());
            }
            Event::End(TagEnd::Table) if is_report_index => break,
            Event::End(TagEnd::Table) => in_table = false,
            _ => {}
        }
    }
    links
}

fn summary_destination(destination: &str) -> String {
    if destination.contains("://") || destination.starts_with('#') {
        return destination.to_owned();
    }
    let (path, fragment) = destination
        .split_once('#')
        .map_or((destination, None), |(path, fragment)| {
            (path, Some(fragment))
        });
    let mut components = vec!["docs"];
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            component => components.push(component),
        }
    }
    let mut output = components.join("/");
    if path.ends_with('/') {
        output.push('/');
    }
    fragment.map_or(output.clone(), |fragment| format!("{output}#{fragment}"))
}

/// Canonical index rows recovered from generated navigation, in order, each
/// paired with the Category cell that produced it: a top-level chapter that is
/// not a generated group page, or a member nested directly under one. Report
/// pairs sit below their index chapter and are not canonical rows.
///
/// Navigation carries no prefix chapter, so every chapter sits under a part
/// heading; one that did not would be recovered with the `Summary` heading as
/// its category and fail the comparison rather than pass unnoticed.
fn summary_category_links(markdown: &str) -> Vec<(String, String)> {
    let mut group: Option<String> = None;
    let mut rows = Vec::new();
    for (part, label, destination, depth) in summary_chapters(markdown) {
        match depth {
            1 if destination.starts_with(GENERATED_GROUP_DIR) => group = Some(label),
            1 => {
                group = None;
                rows.push((part, destination));
            }
            2 => {
                if let Some(group) = group.as_deref() {
                    rows.push((format!("{part}{GROUP_SEPARATOR}{group}"), destination));
                }
            }
            _ => {}
        }
    }
    rows
}

fn strict_lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

#[test]
fn staged_pages_tree_is_clean_and_every_rendered_local_link_resolves() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("creates staging destination");
    let status = Command::new("python3")
        .arg(root.join("scripts/build-docs-site.py"))
        .args(["--source", root.to_str().unwrap(), "--stage"])
        .arg(temp.path())
        .status()
        .expect("runs Pages staging script");
    assert!(status.success(), "staging command succeeds");

    let staged = temp.path().join("src");
    assert!(
        staged.join("SUMMARY.md").is_file(),
        "generator writes SUMMARY.md"
    );
    assert!(
        !temp.path().join("book").exists(),
        "staging never writes generated HTML"
    );

    let link_errors = validate_staged_links(&staged);
    assert!(
        link_errors.is_empty(),
        "staged Markdown links and anchors resolve: {link_errors:#?}"
    );
}

#[test]
fn staged_anchor_validation_rejects_missing_same_and_cross_page_fragments() {
    let temp = tempfile::tempdir().expect("creates staged fixture");
    let staged = temp.path();
    std::fs::write(
        staged.join("same.md"),
        "# Same page\n\n[missing](#not-here)\n",
    )
    .expect("writes same-page fixture");
    std::fs::write(
        staged.join("target.md"),
        "# Punctuation & `code`\n\n## Repeat\n\n## Repeat\n",
    )
    .expect("writes cross-page target");
    std::fs::write(
        staged.join("cross.md"),
        "[valid](target.md#punctuation--code) [deduped](target.md#repeat-1) [missing](target.md#gone) [missing file](absent.md)\n",
    )
    .expect("writes cross-page fixture");

    let errors = validate_staged_links(staged);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("same.md") && error.contains("#not-here")),
        "same-page missing fragment fails: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("cross.md") && error.contains("target.md#gone")),
        "cross-page missing fragment fails: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("cross.md") && error.contains("absent.md")),
        "plain missing target fails: {errors:?}"
    );
    assert_eq!(
        errors.len(),
        3,
        "valid slug and duplicate fragments pass: {errors:?}"
    );
}

#[test]
fn staged_link_validation_refuses_existing_escape_and_keeps_docs_to_root_links() {
    let temp = tempfile::tempdir().expect("creates staged containment fixture");
    let staged = temp.path().join("src");
    std::fs::create_dir_all(staged.join("docs")).expect("creates staged docs directory");
    std::fs::write(staged.join("README.md"), "# Staged root\n").expect("writes staged root page");
    std::fs::write(
        staged.join("docs/guide.md"),
        "[root](../README.md) [escape](../../book.toml) [rooted](/README.md) [backslash](..\\README.md) [drive](C:/README.md)\n",
    )
    .expect("writes staged containment page");
    std::fs::write(temp.path().join("book.toml"), "outside staged source\n")
        .expect("writes existing outside target");

    let errors = validate_staged_links(&staged);
    assert_eq!(
        errors.len(),
        4,
        "the legitimate docs-to-root link resolves while every non-URL or escaping path fails: {errors:?}"
    );
    let normalized_errors: Vec<String> = errors
        .iter()
        .map(|error| error.replace('\\', "/"))
        .collect();
    assert!(
        normalized_errors
            .iter()
            .any(|error| error.contains("docs/guide.md")
                && error.contains("../../book.toml")
                && error.contains("outside staged source")),
        "existing target outside src is rejected: {normalized_errors:?}"
    );
    for destination in ["/README.md", "..\\README.md", "C:/README.md"] {
        let normalized_destination = destination.replace('\\', "/");
        assert!(
            normalized_errors
                .iter()
                .any(|error| error.contains(&normalized_destination)
                    && error.contains("non-URL local path")),
            "rooted, backslash, and drive destinations fail closed: {normalized_errors:?}"
        );
    }
}

#[test]
fn external_proxy_preserves_safe_bracket_and_backslash_labels_and_exact_url() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("creates proxy fixture");
    let source = temp.path().join("source");
    let stage = temp.path().join("stage");
    let label = r"safe [ ] and \ label";
    let destination = "https://example.test/reference?query=exact";
    write_external_index_fixture(&source, label, destination);
    assert!(
        Command::new("python3")
            .arg(root.join("scripts/build-docs-site.py"))
            .args(["--source", source.to_str().unwrap(), "--stage"])
            .arg(&stage)
            .status()
            .expect("stages external proxy fixture")
            .success()
    );

    let summary = std::fs::read_to_string(stage.join("src/SUMMARY.md")).expect("reads summary");
    assert!(
        rendered_links(&summary)
            .iter()
            .any(|(rendered_label, _)| rendered_label == label),
        "escaped SUMMARY label is one rendered label"
    );
    let proxy = summary_category_links(&summary)
        .into_iter()
        .map(|(_, path)| path)
        .find(|path| path.starts_with("_generated/external/"))
        .expect("SUMMARY uses a generated external proxy");
    let proxy_markdown =
        std::fs::read_to_string(stage.join("src").join(proxy)).expect("reads external proxy");
    assert_eq!(rendered_headings(&proxy_markdown), vec![label.to_owned()]);
    assert_eq!(
        rendered_links(&proxy_markdown),
        vec![(format!("Open {label}"), destination.to_owned())],
        "proxy preserves one rendered label and its exact external URL"
    );
}

#[test]
fn pages_composition_uses_release_at_root_and_main_below_dev() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("creates composition fixture");
    let release = temp.path().join("release");
    let main = temp.path().join("main");
    write_book_fixture(&release, "RELEASE ROOT", "0.4.51");
    write_book_fixture(&main, "MAIN DEVELOPMENT", "0.4.52");

    let output = temp.path().join("site");
    // The release build script is not passed: composition must find it inside the
    // release checkout, the way the Pages workflow invokes it.
    let development_builder = temp.path().join("development-builder.py");
    let release_mdbook = temp.path().join("release-mdbook");
    let development_mdbook = temp.path().join("development-mdbook");
    write_fixture_builder(&release.join("scripts/build-docs-site.py"), "release");
    write_fixture_builder(&development_builder, "development");
    assert!(
        Command::new("python3")
            .arg(root.join("scripts/compose-pages-site.py"))
            .args([
                "--development-builder",
                development_builder.to_str().unwrap(),
                "--release-source",
                release.to_str().unwrap(),
                "--main-source",
                main.to_str().unwrap(),
                "--release-stage",
                temp.path().join("release-stage").to_str().unwrap(),
                "--development-stage",
                temp.path().join("development-stage").to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
                "--release-tag",
                "vfixture",
                "--release-mdbook",
                release_mdbook.to_str().unwrap(),
                "--development-mdbook",
                development_mdbook.to_str().unwrap(),
            ])
            .status()
            .expect("runs Pages composition")
            .success()
    );
    let release_root =
        std::fs::read_to_string(output.join("index.html")).expect("reads release root");
    assert_eq!(
        strict_lines(&release_root),
        [
            "# RELEASE ROOT",
            "builder=release",
            "mdbook=release-mdbook",
            "pin=0.4.51",
            "source-ref=vfixture"
        ],
        "the Pages root is built by the release checkout's own build script and mdBook pin, so a \
         later site shape never rewrites a released tree:\n{release_root}"
    );
    let development_root =
        std::fs::read_to_string(output.join("dev/index.html")).expect("reads development subtree");
    assert_eq!(
        strict_lines(&development_root),
        [
            "# MAIN DEVELOPMENT",
            "builder=development",
            "mdbook=development-mdbook",
            "pin=0.4.52",
            "source-ref=main"
        ],
        "the /dev subtree uses current main and its independent mdBook pin:\n{development_root}"
    );
    assert_eq!(
        strict_lines(
            &std::fs::read_to_string(output.join("BUILD-INFO.txt"))
                .expect("reads build routing record"),
        ),
        ["Release root: vfixture", "Development subtree: main"],
        "routing record has exactly the expected semantic lines",
    );
}

fn release_eligibility_fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("creates release eligibility fixture");
    let repository = temp.path().join("releases");
    std::fs::create_dir(&repository).expect("creates fixture repository");
    assert!(
        Command::new("git")
            .args(["init", "--quiet", repository.to_str().unwrap()])
            .status()
            .expect("initializes release fixture")
            .success()
    );
    git(
        &repository,
        &["config", "user.email", "pages-test@example.invalid"],
    );
    git(&repository, &["config", "user.name", "Pages test"]);
    std::fs::write(repository.join("README.md"), "legacy release\n")
        .expect("writes legacy revision");
    git(&repository, &["add", "README.md"]);
    git(&repository, &["commit", "--quiet", "-m", "legacy release"]);
    git(&repository, &["tag", "vlegacy"]);
    std::fs::write(repository.join(".mdbook-version"), "0.4.52\n").expect("adds Pages pin");
    git(&repository, &["add", ".mdbook-version"]);
    git(
        &repository,
        &["commit", "--quiet", "-m", "Pages foundation"],
    );
    git(&repository, &["tag", "vpages"]);

    temp
}

fn release_tag_has_pages_pin(repository: &Path, tag: &str) -> bool {
    Command::new("git")
        .args([
            "-C",
            repository.to_str().expect("fixture path is UTF-8"),
            "cat-file",
            "-e",
            &format!("{tag}:.mdbook-version"),
        ])
        .output()
        .expect("queries tag Pages eligibility")
        .status
        .success()
}

#[test]
fn release_eligibility_policy_and_workflow_invocation_are_cross_platform() {
    let temp = release_eligibility_fixture();
    let repository = temp.path().join("releases");
    assert!(!release_tag_has_pages_pin(&repository, "vlegacy"));
    assert!(release_tag_has_pages_pin(&repository, "vpages"));

    let workflow = std::fs::read_to_string(repo_root().join(".github/workflows/docs-pages.yml"))
        .expect("reads Pages workflow");
    assert!(
        workflow.contains(
            "eligibility=\"$(bash scripts/check-pages-release-eligibility.sh \"$tag\")\""
        ) && workflow.contains("echo \"$eligibility\" >> \"$GITHUB_OUTPUT\"")
            && workflow.contains("install_mdbook ../animsmith-pages-release \"$RUNNER_TEMP/animsmith-pages-release-mdbook\"")
            && workflow.contains("install_mdbook . \"$RUNNER_TEMP/animsmith-pages-development-mdbook\"")
            && workflow.contains("--release-mdbook \"$RUNNER_TEMP/animsmith-pages-release-mdbook/bin/mdbook\"")
            && workflow.contains("--development-mdbook \"$RUNNER_TEMP/animsmith-pages-development-mdbook/bin/mdbook\"")
            && workflow.contains("runs-on: ubuntu-latest"),
        "the Ubuntu Pages workflow consumes the eligibility helper and routes independent pins to each build"
    );
}

#[cfg(unix)]
#[test]
fn unix_pages_runtime_executes_the_exact_eligibility_helper() {
    const ELIGIBILITY_HELPER: &str =
        include_str!("../../../scripts/check-pages-release-eligibility.sh");
    // Pages runs on Ubuntu. Windows CI pins the same policy above with Git
    // object queries and the workflow invocation contract, not a Bash lookup.
    let temp = release_eligibility_fixture();
    let repository = temp.path().join("releases");

    let eligibility = |tag: &str| {
        let output = Command::new("bash")
            .args([
                "-c",
                ELIGIBILITY_HELPER,
                "check-pages-release-eligibility.sh",
                tag,
            ])
            .current_dir(&repository)
            .output()
            .expect("runs release eligibility helper");
        assert!(
            output.status.success(),
            "eligibility helper for {tag:?} exits successfully; status={:?}, stdout={:?}, stderr={:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        String::from_utf8(output.stdout).expect("eligibility output is UTF-8")
    };
    assert_eq!(strict_lines(&eligibility("vlegacy")), ["available=false"]);
    assert_eq!(strict_lines(&eligibility("vpages")), ["available=true"]);
}

#[test]
fn summary_is_deterministic_and_has_the_public_information_architecture() {
    let root = repo_root();
    let first = tempfile::tempdir().expect("first staging destination");
    let second = tempfile::tempdir().expect("second staging destination");
    for destination in [first.path(), second.path()] {
        assert!(
            Command::new("python3")
                .arg(root.join("scripts/build-docs-site.py"))
                .args(["--source", root.to_str().unwrap(), "--stage"])
                .arg(destination)
                .status()
                .expect("runs Pages staging script")
                .success()
        );
    }
    let first_summary =
        std::fs::read_to_string(first.path().join("src/SUMMARY.md")).expect("first summary");
    let second_summary =
        std::fs::read_to_string(second.path().join("src/SUMMARY.md")).expect("second summary");
    assert_eq!(first_summary, second_summary, "navigation is deterministic");

    let report_index = std::fs::read_to_string(root.join("docs/reports/README.md"))
        .expect("reads canonical reports index");
    let expected_report_links = canonical_report_pair_links(&report_index)
        .into_iter()
        .enumerate()
        .map(|(index, destination)| {
            (
                format!("docs/reports/{destination}"),
                if index % 2 == 0 { 3 } else { 4 },
            )
        })
        .collect::<Vec<_>>();
    let generated_links: Vec<(String, usize)> = summary_chapters(&first_summary)
        .into_iter()
        .map(|(_, _, destination, depth)| (destination, depth))
        .collect();
    let reports_position = generated_links
        .iter()
        .position(|(destination, depth)| destination == "docs/reports/README.md" && *depth == 2)
        .expect("summary nests the reports index inside its group");
    let report_links_end = reports_position + 1 + expected_report_links.len();
    assert_eq!(
        &generated_links[reports_position + 1..report_links_end],
        expected_report_links,
        "every report/evidence pair is nested in canonical table order so mdBook publishes it"
    );
    let index =
        std::fs::read_to_string(root.join("docs/README.md")).expect("reads canonical index");
    let index_rows = canonical_index_rows(&index);

    // The report block ends exactly where the canonical table says it does: the
    // next chapter is the following row, or the group chapter that row opens.
    let reports_row = index_rows
        .iter()
        .position(|(_, _, destination)| destination == "reports/README.md")
        .expect("the canonical index rows the reports index");
    let (next_category, next_label, _) = index_rows
        .get(reports_row + 1)
        .expect("a canonical row follows the reports index");
    let group_of = |category: &str| {
        category
            .split_once(GROUP_SEPARATOR)
            .map(|(_, group)| group.to_owned())
    };
    let expected_next = match group_of(next_category) {
        Some(group) if Some(&group) != group_of(&index_rows[reports_row].0).as_ref() => (group, 1),
        Some(_) => (next_label.clone(), 2),
        None => (next_label.clone(), 1),
    };
    let (_, next_chapter, _, next_depth) = &summary_chapters(&first_summary)[report_links_end];
    assert_eq!(
        (next_chapter.clone(), *next_depth),
        expected_next,
        "the chapter after the complete report pair sequence is exactly the canonical table's \
         next entry, so no pair is orphaned"
    );

    let expected: Vec<(String, String)> = index_rows
        .iter()
        .map(|(category, _, destination)| {
            let destination = if destination.contains("://") {
                destination.clone()
            } else {
                summary_destination(destination)
            };
            (category.clone(), destination)
        })
        .collect();
    let generated_rows = summary_category_links(&first_summary);
    let generated = generated_rows
        .into_iter()
        .map(|(category, destination)| {
            if !destination.starts_with("_generated/external/") {
                return (category, destination);
            }
            assert!(
                destination.ends_with(".md"),
                "proxy is a local Markdown page: {destination}"
            );
            let proxy_path = first.path().join("src").join(&destination);
            assert!(
                proxy_path.is_file(),
                "external proxy is staged: {destination}"
            );
            let proxy_links =
                links(&std::fs::read_to_string(&proxy_path).expect("reads external proxy"));
            assert_eq!(proxy_links.len(), 1, "external proxy has one outbound link");
            (
                category,
                proxy_links
                    .into_iter()
                    .next()
                    .expect("proxy link is present"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        generated, expected,
        "SUMMARY.md preserves every canonical index row's category and global order, resolving external proxies to their exact destinations"
    );
    assert!(
        expected
            .iter()
            .any(|(_, destination)| destination.contains("://")),
        "fixture includes an external canonical index row"
    );
    for path in walkdir(&first.path().join("src")) {
        assert!(
            !path
                .file_name()
                .expect("staged filename")
                .to_string_lossy()
                .chars()
                .any(|character| "<>:\"|?*".contains(character) || character.is_control()),
            "staged Pages source has artifact-safe path components: {path:?}"
        );
    }

    assert_eq!(
        rendered_headings(&first_summary),
        ["Summary", "Start", "Symptoms", "Workflows", "More"],
        "the generated parts are exactly the canonical ones, in canonical order"
    );
}

/// A generated group chapter and the sidebar members nested under it.
struct GroupChapter {
    title: String,
    page: String,
    members: Vec<(String, String)>,
}

#[test]
fn every_generated_group_chapter_publishes_exactly_its_canonical_members() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("creates staging destination");
    assert!(
        Command::new("python3")
            .arg(root.join("scripts/build-docs-site.py"))
            .args(["--source", root.to_str().unwrap(), "--stage"])
            .arg(temp.path())
            .status()
            .expect("runs Pages staging script")
            .success()
    );
    let staged = temp.path().join("src");
    let summary =
        std::fs::read_to_string(staged.join("SUMMARY.md")).expect("reads generated summary");

    let chapters = summary_chapters(&summary);
    let mut groups: Vec<GroupChapter> = Vec::new();
    let mut open = None;
    for (_, label, destination, depth) in chapters.clone() {
        match depth {
            1 if destination.starts_with(GENERATED_GROUP_DIR) => {
                open = Some(groups.len());
                groups.push(GroupChapter {
                    title: label,
                    page: destination,
                    members: Vec::new(),
                });
            }
            1 => open = None,
            2 => {
                if let Some(index) = open {
                    groups[index].members.push((label, destination));
                }
            }
            _ => {}
        }
    }
    assert_eq!(
        groups
            .iter()
            .map(|group| group.title.as_str())
            .collect::<Vec<_>>(),
        [
            "Engine profiles",
            "Advanced workflows",
            "Reference",
            "Pack evaluations",
            "Rust integration",
            "Project and contributing",
        ],
        "the generated group chapters are exactly the canonical groups, in canonical order"
    );

    // Report pairs are navigation detail: they must sit inside the Pack
    // evaluations group and be nested, so level-0 folding hides them by default.
    let expected_pairs = canonical_report_pair_links(
        &std::fs::read_to_string(root.join("docs/reports/README.md"))
            .expect("reads canonical reports index"),
    )
    .len();
    let pack = chapters
        .iter()
        .position(|(_, label, destination, depth)| {
            label == "Pack evaluations"
                && destination.starts_with(GENERATED_GROUP_DIR)
                && *depth == 1
        })
        .expect("the summary opens a Pack evaluations group chapter");
    let group_end = chapters[pack + 1..]
        .iter()
        .position(|(_, _, _, depth)| *depth == 1)
        .map_or(chapters.len(), |offset| pack + 1 + offset);
    let pairs: Vec<usize> = chapters
        .iter()
        .enumerate()
        .filter(|(_, (_, _, destination, _))| {
            destination.starts_with("docs/reports/") && destination != "docs/reports/README.md"
        })
        .map(|(position, _)| position)
        .collect();
    assert_eq!(
        pairs.len(),
        expected_pairs,
        "every canonical report and evidence page is published as a chapter"
    );
    for position in pairs {
        let (_, _, destination, depth) = &chapters[position];
        assert!(
            position > pack && position < group_end && *depth >= 2,
            "{destination} is nested at depth {depth} inside the Pack evaluations group"
        );
    }

    for GroupChapter {
        title,
        page,
        members,
    } in groups
    {
        assert!(!members.is_empty(), "the {title} group chapter has members");
        let page = staged.join(&page);
        let markdown = std::fs::read_to_string(&page).expect("reads generated group page");
        assert_eq!(
            rendered_headings(&markdown),
            vec![title.clone()],
            "a group page is titled by its canonical group name"
        );
        let published: Vec<(String, PathBuf)> = first_item_links(&markdown)
            .into_iter()
            .map(|(label, destination)| {
                let local = destination.split('#').next().unwrap_or_default();
                (
                    label,
                    staged_local_target(&staged, &page, local).expect("group link stays staged"),
                )
            })
            .collect();
        let expected: Vec<(String, PathBuf)> = members
            .iter()
            .map(|(label, destination)| (label.clone(), staged.join(destination)))
            .collect();
        assert_eq!(
            published, expected,
            "the {title} group page lists exactly its sidebar members, in order, \
             pointing at the same staged pages"
        );
    }
}

/// Every reference the landing page asks a browser to follow or fetch:
/// its navigation and body links, the theme and font stylesheets, the
/// favicon, the charts, and any script it loads.
const LANDING_REFERENCES: [(&str, &str); 5] = [
    ("a", "href"),
    ("link", "href"),
    ("img", "src"),
    ("iframe", "src"),
    ("script", "src"),
];

/// The tracked repository file a landing-page reference resolves to, given
/// that the page is published as the artifact root: `docs/<page>.html` is a
/// rendered chapter, `docs/<dir>/index.html` is that directory's index
/// chapter, and everything else is a file mdBook copies verbatim from the
/// staged source or from the theme.
fn landing_reference_source(destination: &str) -> String {
    let path = destination.split(['#', '?']).next().unwrap_or_default();
    match path.strip_suffix(".html") {
        Some(page) if path.starts_with("docs/") && !path.starts_with("docs/visuals/") => {
            match page.strip_suffix("/index") {
                Some(directory) => format!("{directory}/README.md"),
                None => format!("{page}.md"),
            }
        }
        _ => match path {
            "favicon.svg" | "favicon.png" => format!("docs/site/{path}"),
            _ => match path.strip_prefix("theme/") {
                Some(asset) => format!("docs/site/{asset}"),
                None => match path.strip_prefix("fonts/") {
                    Some(asset) => format!("docs/site/fonts/{asset}"),
                    None => path.to_owned(),
                },
            },
        },
    }
}

/// The site's front door replaces the artifact's root index, so the canonical
/// index must leave that route free and every reference the page makes must
/// name something the build publishes. mdBook rewrites nothing in this page:
/// it is copied verbatim, so a stale path here is a 404 for the first page a
/// reader ever sees.
#[test]
fn the_tracked_landing_page_takes_the_root_index_and_every_reference_resolves() {
    let root = repo_root();
    let index = std::fs::read_to_string(root.join("docs/README.md")).expect("reads docs index");
    assert!(
        !canonical_index_rows(&index)
            .iter()
            .any(|(_, _, destination)| destination.split('#').next() == Some("../README.md")),
        "the canonical index must not row the root README, with or without a `#fragment`: \
         mdBook renders it to the same book/index.html the landing page is published at"
    );

    let landing =
        std::fs::read_to_string(root.join("docs/site/landing.html")).expect("reads landing page");
    let references = docs_html::html_references(&landing, &LANDING_REFERENCES);
    assert!(
        references.len() >= 15,
        "the landing page routes the site: {references:#?}"
    );

    let mut missing = Vec::new();
    let mut external = Vec::new();
    for (tag, destination) in references {
        if destination.starts_with("https://") || destination.starts_with("http://") {
            external.push((tag, destination));
            continue;
        }
        let source = landing_reference_source(&destination);
        if !root.join(&source).exists() {
            missing.push(format!("{tag} {destination} -> {source}"));
        }
    }
    assert!(
        missing.is_empty(),
        "every landing-page reference resolves to a tracked source: {missing:#?}"
    );
    assert!(
        external.iter().all(|(tag, _)| tag == "a"),
        "the published page fetches nothing from a third party: {external:#?}"
    );
}

/// Every line of the front door's CLI transcript. The page shows tool
/// output in `<div class="find">`; a reader reads those as the lines the
/// command prints.
fn landing_transcript_lines(html: &str) -> Vec<String> {
    const OPEN: &str = "<div class=\"find\">";
    let mut lines = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find(OPEN) {
        let body = &rest[start + OPEN.len()..];
        let end = body.find("</div>").unwrap_or(body.len());
        lines.extend(
            body[..end]
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned),
        );
        rest = &body[end..];
    }
    lines
}

/// The front door quotes the tool, so it must quote it exactly. Every
/// transcript line it shows has to appear in `docs/first-lint.md`'s
/// gated `console` fence, which `start_docs.rs` runs against the built
/// binary — a hand-trimmed finding on the first page a reader sees is a
/// promise nothing verifies.
#[test]
fn the_front_door_transcript_is_quoted_from_the_gated_first_lint_page() {
    let root = repo_root();
    let landing =
        std::fs::read_to_string(root.join("docs/site/landing.html")).expect("reads landing page");
    let quoted = landing_transcript_lines(&landing);
    assert!(
        !quoted.is_empty(),
        "the front door shows the tool's own output"
    );

    let page = "docs/first-lint.md";
    let markdown = std::fs::read_to_string(root.join(page)).expect("reads the first-lint page");
    let documented: BTreeSet<String> = docs_markdown::fenced_blocks(&markdown, "console")
        .iter()
        .flat_map(|block| block.lines())
        .map(|line| line.trim().to_owned())
        .collect();

    for line in quoted {
        assert!(
            documented.contains(&line),
            "the front door shows {line:?}, which {page} does not document"
        );
    }
}

/// Extract every `url(...)` target from a stylesheet without adding a dependency.
fn css_urls(css: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = css;
    while let Some(start) = rest.find("url(") {
        rest = &rest[start + "url(".len()..];
        let Some(end) = rest.find(')') else { break };
        targets.push(
            rest[..end]
                .trim()
                .trim_matches(|character| character == '"' || character == '\'')
                .to_owned(),
        );
        rest = &rest[end + 1..];
    }
    targets
}

/// The published book must not fetch anything from a third party: the theme is
/// tracked in full, so a reader's browser only ever asks the Pages origin.
#[test]
fn the_tracked_theme_references_no_external_resources() {
    let site = repo_root().join("docs/site");
    let script = std::fs::read_to_string(site.join(THEME_SCRIPT)).expect("reads theme bridge");
    for external in ["http://", "https://", "//cdn", "import(", "importScripts"] {
        assert!(
            !script.contains(external),
            "{THEME_SCRIPT} reaches outside the published origin: {external}"
        );
    }
    for relative in ["animsmith.css", "fonts/fonts.css"] {
        let css = std::fs::read_to_string(site.join(relative)).expect("reads tracked stylesheet");
        let compact: String = css.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            !compact.contains("@import"),
            "{relative} imports another stylesheet"
        );
        for external in [
            "url(http",
            "url(\"http",
            "url('http",
            "url(//",
            "url(\"//",
            "url('//",
        ] {
            assert!(
                !compact.contains(external),
                "{relative} fetches an external resource: {external}"
            );
        }
    }

    let fonts = site.join("fonts/fonts.css");
    let targets = css_urls(&std::fs::read_to_string(&fonts).expect("reads font stylesheet"));
    assert!(
        !targets.is_empty(),
        "the tracked font stylesheet declares at least one font file"
    );
    for target in targets {
        assert!(
            fonts
                .parent()
                .expect("fonts.css has a directory")
                .join(&target)
                .is_file(),
            "fonts.css names {target}, which must ship next to it"
        );
    }
}

/// The theme override directory is staged from tracked `docs/site` files and is
/// never publishable source. A checkout without those assets — every release tag
/// predating them — still builds, so the stylesheet is wired exactly when the
/// checkout tracks it.
#[test]
fn tracked_site_assets_stage_as_the_theme_and_never_as_published_source() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("creates staging destination");
    assert!(
        Command::new("python3")
            .arg(root.join("scripts/build-docs-site.py"))
            .args(["--source", root.to_str().unwrap(), "--stage"])
            .arg(temp.path())
            .status()
            .expect("runs Pages staging script")
            .success()
    );

    let listed = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["ls-files", "-z", "--", "docs/site"])
        .output()
        .expect("lists tracked site assets");
    assert!(listed.status.success(), "git ls-files succeeds");
    let expected: BTreeSet<String> = String::from_utf8(listed.stdout)
        .expect("tracked paths are UTF-8")
        .split_terminator('\0')
        .filter(|tracked| {
            // The redirect map is build configuration and the landing page is
            // published as the artifact root; neither is a theme asset.
            !matches!(
                *tracked,
                "docs/site/redirects.toml" | "docs/site/landing.html"
            )
        })
        .map(|tracked| format!("theme/{}", &tracked["docs/site/".len()..]))
        .collect();

    let theme = temp.path().join("theme");
    let staged: BTreeSet<String> = if theme.is_dir() {
        walkdir(&theme)
            .into_iter()
            .map(|path| {
                format!(
                    "theme/{}",
                    path.strip_prefix(&theme)
                        .expect("theme asset is staged")
                        .to_string_lossy()
                        .replace('\\', "/")
                )
            })
            .collect()
    } else {
        BTreeSet::new()
    };
    assert_eq!(
        staged, expected,
        "the staged theme mirrors tracked docs/site without its redirect map or landing page"
    );
    assert!(
        !temp.path().join("src/docs/site").exists(),
        "theme assets are never published as book source"
    );

    let book = std::fs::read_to_string(temp.path().join("book.toml")).expect("reads book.toml");
    for key in [
        "[output.html.fold]\nenable = true\nlevel = 0\n",
        "default-theme = \"light\"\n",
        "preferred-dark-theme = \"navy\"\n",
    ] {
        assert!(book.contains(key), "book.toml carries {key:?}: {book}");
    }
    assert!(
        book.contains("additional-css = [\"theme/animsmith.css\"]"),
        "book.toml wires the tracked stylesheet: {book}"
    );
    assert!(
        book.contains("additional-js = [\"theme/animsmith.js\"]"),
        "book.toml wires the tracked theme bridge: {book}"
    );
    assert!(
        temp.path().join("theme").join(THEME_SCRIPT).is_file(),
        "the theme bridge is staged beside the stylesheet"
    );
}

/// The front door is a hand-authored page at the artifact root that no
/// chapter links, so without the tracked script a reader who follows the
/// sidebar into the book has no way back to it. The script is what supplies
/// that link, in the sidebar and on the title in the top bar, and the
/// stylesheet is what keeps the title's link usable once the logo is drawn
/// over its text.
///
/// What the script *does* to a page is executed rather than read:
/// `scripts/test-theme-bridge.js` builds mdBook's own chrome and asserts the
/// sidebar gains exactly one Home entry resolving through `path_to_root`.
/// This test keeps the asset, its wiring and its stylesheet in place.
#[test]
fn the_site_script_gives_every_page_a_link_back_to_the_front_door() {
    let root = repo_root();
    let script =
        std::fs::read_to_string(root.join("docs/site").join(THEME_SCRIPT)).expect("reads script");
    for required in [
        // mdBook's own site-root prefix, which is what makes one href work
        // from every depth, and the front door it resolves to.
        "path_to_root",
        "index.html",
        // The two places the link is written, and the label a reader reads.
        ".sidebar .chapter",
        ".menu-title",
        "\"Home\"",
        // The class both links carry, which is how the script recognises its
        // own work and how the stylesheet finds it.
        "as-home",
    ] {
        assert!(
            script.contains(required),
            "{THEME_SCRIPT} must keep {required:?}"
        );
    }

    let stylesheet =
        std::fs::read_to_string(root.join("docs/site/animsmith.css")).expect("reads stylesheet");
    for required in [".menu-title a.as-home", ".chapter li.as-home-item"] {
        assert!(
            stylesheet.contains(required),
            "the tracked stylesheet must style {required:?}"
        );
    }
}

/// The bridge's own selector, mirrored here so the pages and the script
/// cannot drift apart: a report document under `docs/visuals/`, in either
/// the relative spelling the Markdown carries or the site-absolute one
/// staging rewrites it to.
fn selects_report(source: &str) -> bool {
    let path = source.split('#').next().unwrap_or_default();
    path.ends_with(".html")
        && (path.contains("/docs/visuals/")
            || path.starts_with("visuals/")
            || path.contains("/visuals/"))
}

/// The theme bridge is a tracked asset like the stylesheet, so the staged
/// theme carries it and a published page never needs an inline script.
///
/// What the bridge *does* to a fragment is executed rather than read:
/// `scripts/test-theme-bridge.js` runs it against a synthetic page under
/// `just report-browser`, beside the generated viewers it drives. This test
/// keeps the asset, its selector and that wiring in place.
#[test]
fn the_theme_bridge_is_tracked_and_pins_the_embedded_report_theme() {
    let root = repo_root();
    let recipes = std::fs::read_to_string(root.join("justfile")).expect("reads the recipes");
    assert!(
        recipes.contains("node scripts/test-theme-bridge.js"),
        "the browser harness recipe must execute the theme bridge's contract"
    );
    let listed = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["ls-files", "--", "docs/site/animsmith.js"])
        .output()
        .expect("lists the tracked theme bridge");
    assert!(listed.status.success(), "git ls-files succeeds");
    assert_eq!(
        String::from_utf8_lossy(&listed.stdout).trim(),
        "docs/site/animsmith.js",
        "the theme bridge must be tracked for the build to stage it"
    );

    let script =
        std::fs::read_to_string(root.join("docs/site").join(THEME_SCRIPT)).expect("reads bridge");
    for required in [
        // The book's theme classes it maps, both directions.
        "navy",
        "coal",
        "ayu",
        "light",
        "rust",
        // The fragment key the report viewers read, and the observer that
        // re-applies it when mdBook swaps the class on <html>.
        "theme",
        "MutationObserver",
        "attributeFilter",
    ] {
        assert!(
            script.contains(required),
            "{THEME_SCRIPT} must keep {required:?}"
        );
    }
    assert!(
        !script.contains("contentDocument") && !script.contains("contentWindow"),
        "{THEME_SCRIPT} rewrites its own src rather than reading into the frame"
    );

    // Every page that embeds a report must be reachable by the bridge's own
    // rule, in both spellings a frame source is ever written in: the
    // relative one the repository Markdown carries, and the site-absolute
    // one staging rewrites it to for the published page.
    for spelling in [
        "../visuals/walk.report.html",
        "/animsmith/docs/visuals/walk.report.html",
    ] {
        assert!(
            selects_report(spelling),
            "{THEME_SCRIPT} must select a frame written as {spelling}"
        );
    }
    assert!(
        !selects_report("/animsmith/docs/cli.html") && !selects_report("../visuals/walk.svg"),
        "{THEME_SCRIPT} selects report documents only"
    );

    let mut embedded = 0usize;
    for page in markdown_files(&root.join("docs")) {
        let markdown = std::fs::read_to_string(&page).expect("reads documentation page");
        let mut rest = markdown.as_str();
        while let Some(offset) = rest.find("<iframe src=\"") {
            rest = &rest[offset + "<iframe src=\"".len()..];
            let source = &rest[..rest.find('"').expect("iframe src is quoted")];
            let path = source.split('#').next().unwrap_or_default();
            assert!(
                selects_report(path),
                "{} embeds {source}, which the theme bridge would not recognise",
                page.display()
            );
            for root in ["/animsmith/docs/", "/animsmith/dev/docs/"] {
                let staged = format!("{root}{}", path.trim_start_matches("../"));
                assert!(
                    selects_report(&staged),
                    "{} embeds {source}, which the bridge would lose once staging writes \
                     it as {staged}",
                    page.display()
                );
            }
            embedded += 1;
        }
    }
    assert!(
        embedded >= 9,
        "the symptom pages must keep embedding their reports, found {embedded}"
    );
}
