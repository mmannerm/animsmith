//! Contract tests for the mdBook staging boundary.  The book generator may
//! parse the canonical index mechanically, but staged Markdown is validated
//! with pulldown-cmark so the check covers what a renderer actually sees.

use pulldown_cmark::{Event, Options, Parser, Tag};
use std::collections::BTreeSet;
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

    for page in markdown_files(&staged) {
        let text = std::fs::read_to_string(&page).expect("reads staged Markdown");
        for destination in links(&text) {
            if destination.starts_with('#')
                || destination.contains("://")
                || destination.starts_with("mailto:")
            {
                continue;
            }
            let local = destination.split('#').next().unwrap_or_default();
            assert!(
                page.parent()
                    .expect("staged page parent")
                    .join(local)
                    .exists(),
                "{} renders a link to missing staged target {destination}",
                page.strip_prefix(&staged)
                    .expect("page is staged")
                    .display()
            );
        }
    }
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
