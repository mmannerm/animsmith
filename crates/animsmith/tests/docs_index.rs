//! Drift gate for the documentation index: every top-level page in
//! `docs/` must be linked from a row of the Document index table in
//! `docs/README.md`, so adding a doc page means adding an index-table
//! row. The table is located by parsing the markdown with
//! `pulldown-cmark` (the parser rustdoc uses) rather than by pattern
//! matching, so fenced or indented decoy tables, delimiter-less fake
//! headers, and malformed link fragments cannot satisfy the gate.
//! Link *targets* (does the linked file exist?) stay covered by
//! `scripts/check-github-community-files.sh`.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// Link destinations (fragments stripped) inside the first table whose
/// leading header cell is exactly `Document` — the index table's
/// declared shape. Rendered-link destinations only: text that merely
/// looks like a link never produces a `Tag::Link` event.
fn document_index_link_targets(markdown: &str) -> BTreeSet<String> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);

    let mut targets = BTreeSet::new();
    let mut in_head = false;
    let mut head_first_cell: Option<String> = None;
    let mut collecting_first_cell = false;
    let mut is_index_table = false;

    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Start(Tag::Table(_)) => {
                is_index_table = false;
                head_first_cell = None;
            }
            Event::Start(Tag::TableHead) => in_head = true,
            Event::End(TagEnd::TableHead) => {
                in_head = false;
                is_index_table = head_first_cell.as_deref() == Some("Document");
            }
            Event::Start(Tag::TableCell) if in_head && head_first_cell.is_none() => {
                collecting_first_cell = true;
                head_first_cell = Some(String::new());
            }
            Event::End(TagEnd::TableCell) => collecting_first_cell = false,
            Event::Text(text) if collecting_first_cell => {
                if let Some(cell) = head_first_cell.as_mut() {
                    cell.push_str(&text);
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) if is_index_table => {
                let target = dest_url.split('#').next().unwrap_or_default();
                targets.insert(target.to_owned());
            }
            Event::End(TagEnd::Table) if is_index_table => break,
            _ => {}
        }
    }
    targets
}

#[test]
fn every_top_level_docs_page_has_an_index_table_row() {
    let index = std::fs::read_to_string(repo_path("docs/README.md")).expect("reads docs index");
    let targets = document_index_link_targets(&index);
    assert!(
        !targets.is_empty(),
        "docs/README.md must carry the Document index table"
    );

    for entry in std::fs::read_dir(repo_path("docs")).expect("lists docs/") {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8 doc name")
            .to_owned();
        if name == "README.md" {
            continue;
        }
        assert!(
            targets.contains(&name),
            "docs/README.md must carry an index-table row linking {name}; \
             table links: {targets:?}"
        );
    }
}

/// The mutation catalog the shell oracle accumulated across audit
/// rounds, now pinned against the parser: only rendered links inside
/// the Document table count.
#[test]
fn oracle_counts_only_rendered_links_inside_the_document_table() {
    let fixture = "# Documentation\n\n\
        ```text\n\
        | Document | Use it to… |\n\
        |---|---|\n\
        | [backtick-fenced.md](backtick-fenced.md) | fenced decoy |\n\
        ```\n\n\
        ~~~text\n\
        | Document | Use it to… |\n\
        |---|---|\n\
        | [tilde-fenced.md](tilde-fenced.md) | fenced decoy |\n\
        ~~~\n\n\
        A paragraph, then an indented code block:\n\n\
        \x20   | Document | Use it to… |\n\
        \x20   |---|---|\n\
        \x20   | [indented.md](indented.md) | indented decoy |\n\n\
        | Document | this header has no delimiter row, so it is prose |\n\
        with [delimiterless.md](delimiterless.md) linked right after.\n\n\
        | Other | Column |\n\
        |---|---|\n\
        | [other-table.md](other-table.md) | wrong table |\n\n\
        | Document | Use it to… |\n\
        |---|---|\n\
        | [real.md](real.md) | a genuine row |\n\
        | [fragment.md](fragment.md#section) | anchor stripped |\n\
        | malformed ](broken.md) cell without an opening bracket |\n\n\
        Prose with a loose [prose.md](prose.md) link.\n\n\
        | standalone pipe line with [loose.md](loose.md) outside any table |\n";

    let targets = document_index_link_targets(fixture);
    let expected: BTreeSet<String> = ["real.md", "fragment.md"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        targets, expected,
        "only rendered links in the Document table count"
    );
}
