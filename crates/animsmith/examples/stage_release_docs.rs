//! Writes the repository's current-version documentation from the
//! workspace manifest: the `package = "X.Y"` dependency snippets in the
//! published READMEs and the embedding guide, and the `"tool"` objects the
//! output examples quote.
//!
//! The inventory, the reader that locates each claim, and the writer that
//! moves it live in `animsmith-testkit`'s
//! [`docs_versions`](animsmith_testkit::docs_versions) module, which this
//! example and the `release_version_docs` gate both drive, so the tool and
//! the gate cannot disagree about what a document claims. Only the located
//! spans are rewritten: a historical version elsewhere on the same page is
//! not a current-version claim and is left alone.
//!
//! Restate the workspace version (the release PR's own step, and a no-op
//! on a clean checkout):
//!   cargo run -p animsmith --example stage_release_docs
//! Stage the release about to be dispatched, which the manifest does not
//! carry yet:
//!   cargo run -p animsmith --example stage_release_docs -- --version <next>

use animsmith_testkit::docs_versions::{Version, stage_release_docs, workspace_version};
use std::path::Path;

const USAGE: &str =
    "usage: cargo run -p animsmith --example stage_release_docs [-- --version X.Y.Z]";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = match requested_version()? {
        Some(version) => version,
        None => workspace_version(&repository)?,
    };

    let changes = stage_release_docs(&repository, target)?;
    if changes.is_empty() {
        println!("current-version documentation already describes {target}");
        return Ok(());
    }
    for change in &changes {
        println!("{change}");
    }
    println!(
        "staged {} current-version claim(s) at {target}",
        changes.len()
    );
    Ok(())
}

/// The version named on the command line, if one was.
///
/// Only `--version X.Y.Z` is accepted: the release line to stage is the
/// one decision this tool takes from its caller, and everything else it
/// needs it reads from the repository.
fn requested_version() -> Result<Option<Version>, String> {
    let mut arguments = std::env::args().skip(1);
    let Some(flag) = arguments.next() else {
        return Ok(None);
    };
    if flag != "--version" {
        return Err(format!("unexpected argument {flag:?}\n{USAGE}"));
    }
    let value = arguments
        .next()
        .ok_or_else(|| format!("--version needs a version\n{USAGE}"))?;
    if let Some(extra) = arguments.next() {
        return Err(format!("unexpected argument {extra:?}\n{USAGE}"));
    }
    Version::parse(&value).map(Some)
}
