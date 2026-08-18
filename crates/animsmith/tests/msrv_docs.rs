//! Drift gate for the MSRV quoted in published crate documentation.
//!
//! `workspace.package.rust-version` in `Cargo.toml` is the single source of
//! truth for the MSRV. Every crate README and crate-level rustdoc header that
//! names a Rust version is a published claim about that number, so a bump in
//! the manifest must not leave a stale version behind in prose.
//!
//! The inventory is discovered, not listed: every `crates/*/README.md` and
//! `crates/*/src/lib.rs` is scanned, so a new crate is covered the day it
//! lands. `DEVELOPMENT.md` states the MSRV in a different shape and is held
//! against the manifest by `scripts/check-github-community-files.sh`.

use std::path::{Path, PathBuf};

/// Inherited from `workspace.package.rust-version` via `rust-version.workspace
/// = true`, so this gate reads the manifest without parsing it.
const MSRV: &str = env!("CARGO_PKG_RUST_VERSION");

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `Rust <version>` mention in `content`, where `<version>` carries at
/// least one dot. The dot requirement keeps edition references ("Rust 2024")
/// out of the scan, since only dotted forms are MSRV claims.
fn quoted_rust_versions(content: &str) -> Vec<String> {
    let mut versions = Vec::new();
    for (index, _) in content.match_indices("Rust ") {
        let tail = &content[index + "Rust ".len()..];
        let end = tail
            .find(|character: char| !character.is_ascii_digit() && character != '.')
            .unwrap_or(tail.len());
        let version = tail[..end].trim_end_matches('.');
        if version.contains('.')
            && version.split('.').all(|part| {
                !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
            })
        {
            versions.push(version.to_owned());
        }
    }
    versions
}

/// Crate READMEs and crate-level rustdoc headers, discovered from `crates/`.
fn published_crate_docs(root: &Path) -> Vec<PathBuf> {
    let mut docs = Vec::new();
    let entries = std::fs::read_dir(root.join("crates")).expect("reads crates/");
    for entry in entries {
        let crate_dir = entry.expect("reads a crates/ entry").path();
        for relative in ["README.md", "src/lib.rs"] {
            let doc = crate_dir.join(relative);
            if doc.is_file() {
                docs.push(doc);
            }
        }
    }
    docs.sort();
    assert!(!docs.is_empty(), "crates/ must contain published docs");
    docs
}

#[test]
fn published_crate_docs_quote_the_manifest_msrv() {
    let root = repo_root();
    let mut stale = Vec::new();
    for doc in published_crate_docs(&root) {
        let content = std::fs::read_to_string(&doc).expect("reads a published crate doc");
        let display = doc
            .strip_prefix(&root)
            .unwrap_or(&doc)
            .display()
            .to_string();
        for version in quoted_rust_versions(&content) {
            if version != MSRV {
                stale.push(format!("{display} quotes Rust {version}"));
            }
        }
    }
    assert!(
        stale.is_empty(),
        "published docs must quote the manifest MSRV Rust {MSRV}: {}",
        stale.join(", ")
    );
}

#[test]
fn scanner_reads_msrv_claims_and_skips_edition_references() {
    assert_eq!(
        quoted_rust_versions("The workspace MSRV is Rust 1.97."),
        ["1.97"],
        "a sentence-final MSRV claim is a version, not a version plus period"
    );
    assert_eq!(
        quoted_rust_versions("MSRV, Rust 1.97.1. Its Rust API is pre-1.0."),
        ["1.97.1"],
        "patch-level claims count and bare `Rust API` prose does not"
    );
    assert!(
        quoted_rust_versions("animsmith uses the Rust 2024 edition").is_empty(),
        "an edition is not an MSRV claim"
    );
}
