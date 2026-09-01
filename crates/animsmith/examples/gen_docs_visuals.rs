//! Regenerates the committed documentation visuals under `docs/visuals/`:
//! the HTML reports the customer pages embed and link, and the standalone
//! SVG charts cut out of them.
//!
//! The manifest and the chart extractor live in `animsmith-testkit`'s
//! [`docs_visuals`](animsmith_testkit::docs_visuals) module, which this
//! example and the guard test (`docs_visuals.rs`) both drive, so the
//! committed pictures can never silently drift from the tool that made
//! them. Every report is rendered by the `animsmith` CLI itself — this
//! example only decides which invocations to run and where their output
//! goes — so a committed report is exactly what a reader gets from the
//! same command on the same committed fixture.
//!
//! Run (writes to the repo's `docs/visuals/`):
//!   cargo run -p animsmith --example gen_docs_visuals
//! Write elsewhere:
//!   cargo run -p animsmith --example gen_docs_visuals -- /some/dir

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| repository.join(animsmith_testkit::docs_visuals::OUTPUT_DIR));
    let working_dir = repository.join(animsmith_testkit::docs_visuals::WORKING_DIR);
    // `cargo build --example` never builds the package's binaries, so the
    // CLI is reached through Cargo rather than through a target-directory
    // path this example would have to guess.
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

    animsmith_testkit::docs_visuals::write_docs_visuals(&out_dir, |arguments| {
        let status = Command::new(&cargo)
            .args(["run", "--quiet", "-p", "animsmith", "--"])
            .args(arguments)
            .current_dir(&working_dir)
            .status()
            .map_err(|error| format!("runs animsmith {}: {error}", arguments.join(" ")))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "animsmith {} exited with {status}",
                arguments.join(" ")
            ))
        }
    })?;

    println!("wrote {}", out_dir.display());
    Ok(())
}
