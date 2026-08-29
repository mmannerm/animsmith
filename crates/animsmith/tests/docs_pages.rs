//! Contract tests for the mdBook staging boundary.  The book generator may
//! parse the canonical index mechanically, but staged Markdown is validated
//! with pulldown-cmark so the check covers what a renderer actually sees.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

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
            let target = if local.is_empty() {
                page.clone()
            } else {
                page.parent().expect("staged page parent").join(local)
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

fn write_book_fixture(root: &Path, marker: &str) {
    std::fs::create_dir_all(root.join("docs")).expect("creates fixture docs directory");
    std::fs::write(root.join(".mdbook-version"), "0.4.52\n").expect("writes mdBook pin");
    std::fs::write(root.join("README.md"), format!("# {marker}\n")).expect("writes root page");
    std::fs::write(
        root.join("docs/README.md"),
        format!("# {marker}\n\n| Document | Use it to… | Category |\n|---|---|---|\n| [Guide](guide.md) | Fixture guide. | Guides |\n"),
    )
    .expect("writes canonical fixture index");
    std::fs::write(root.join("docs/guide.md"), format!("# {marker} guide\n"))
        .expect("writes fixture guide");
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

    assert!(
        validate_staged_links(&staged).is_empty(),
        "staged Markdown links and anchors resolve"
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
        "[valid](target.md#punctuation--code) [deduped](target.md#repeat-1) [missing](target.md#gone)\n",
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
    assert_eq!(
        errors.len(),
        2,
        "valid slug and duplicate fragments pass: {errors:?}"
    );
}

#[test]
fn pages_composition_uses_release_at_root_and_main_below_dev() {
    let root = repo_root();
    let temp = tempfile::tempdir().expect("creates composition fixture");
    let release = temp.path().join("release");
    let main = temp.path().join("main");
    write_book_fixture(&release, "RELEASE ROOT");
    write_book_fixture(&main, "MAIN DEVELOPMENT");

    let output = temp.path().join("site");
    assert!(
        Command::new("python3")
            .arg(root.join("scripts/compose-pages-site.py"))
            .args([
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
            ])
            .status()
            .expect("runs Pages composition")
            .success()
    );
    assert!(
        std::fs::read_to_string(output.join("index.html"))
            .expect("reads release root")
            .contains("RELEASE ROOT"),
        "the Pages root comes from the selected release checkout"
    );
    assert!(
        std::fs::read_to_string(output.join("dev/index.html"))
            .expect("reads development subtree")
            .contains("MAIN DEVELOPMENT"),
        "the /dev subtree comes from current main"
    );
    assert_eq!(
        std::fs::read_to_string(output.join("BUILD-INFO.txt")).expect("reads build routing record"),
        "Release root: vfixture\nDevelopment subtree: main\n"
    );
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

    let headings: BTreeSet<String> = Parser::new_ext(&first_summary, options())
        .filter_map(|event| match event {
            Event::Text(text) => Some(text.into_string()),
            _ => None,
        })
        .collect();
    for category in [
        "Get started",
        "Guides",
        "Reference",
        "Rust integration",
        "Contributing",
        "Project",
        "Research archive",
    ] {
        assert!(headings.contains(category), "SUMMARY.md has {category}");
    }
}
