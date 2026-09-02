//! Drift gate for the documentation index: every top-level page in
//! `docs/` must be linked from a row of the Document index table in
//! `docs/README.md`, so adding a doc page means adding an index-table
//! row. The table is read with the shared parser-backed reader
//! (`animsmith_testkit::docs_markdown`) rather than by pattern matching,
//! so fenced or indented decoy tables, delimiter-less fake headers, and
//! malformed link fragments cannot satisfy the gate; that reader owns
//! the mutation catalog proving it.
//! Link *targets* and `#anchor`s are covered by the sibling gate
//! `docs_links.rs`. Nested dirs (research/, schemas/, visuals/) are
//! outside the top-level indexed set, and preventing a second routing
//! list elsewhere is review policy, not something a gate can prove.
//!
//! A nested directory that publishes customer pages carries the same
//! rule one level down: `docs/symptoms/` owns a sub-index, held to the
//! same completeness by the same helper.
//!
//! Forward constraint for a generated docs site (GitHub Pages/mdBook):
//! its navigation (e.g. SUMMARY.md) must be derived from this index
//! table or a shared manifest — never a second hand-maintained routing
//! list. Note the index also rows pages outside docs/
//! (../examples/README.md, ../CONTRIBUTING.md); a site build must decide
//! link-vs-include for those rather than assume the set is docs/*.md.

use animsmith_testkit::docs_markdown::document_index_targets;
use std::path::PathBuf;

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// Hold one index to its directory: every `.md` page beside it, except
/// the index itself, must have a Document-column row. Returns how many
/// pages were checked, so a caller can prove the directory is not empty.
fn every_page_beside_the_index_has_a_row(index_path: &str, directory: &str) -> usize {
    let index = std::fs::read_to_string(repo_path(index_path))
        .unwrap_or_else(|error| panic!("reads {index_path}: {error}"));
    let targets = document_index_targets(&index);
    assert!(
        !targets.is_empty(),
        "{index_path} must carry the Document index table"
    );

    let mut pages = 0usize;
    for entry in std::fs::read_dir(repo_path(directory))
        .unwrap_or_else(|error| panic!("lists {directory}: {error}"))
    {
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
            "{index_path} must carry an index-table row linking {name}; \
             table links: {targets:?}"
        );
        pages += 1;
    }
    pages
}

#[test]
fn every_top_level_docs_page_has_an_index_table_row() {
    let pages = every_page_beside_the_index_has_a_row("docs/README.md", "docs");
    assert!(pages > 0, "docs/ must publish documentation pages");
}

/// A sub-index owns its directory the way `docs/README.md` owns
/// `docs/`: adding a symptom page means adding its row, so the page is
/// reachable from the index a reader lands on and from the site
/// navigation the build derives from it.
#[test]
fn every_symptom_page_has_a_sub_index_row() {
    let pages = every_page_beside_the_index_has_a_row("docs/symptoms/README.md", "docs/symptoms");
    assert!(pages > 0, "docs/symptoms/ must publish symptom pages");
}
