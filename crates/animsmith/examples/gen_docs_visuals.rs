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
//! same command on the same committed fixture. An invocation whose input
//! is the output of a repair reads it from a throwaway scratch directory
//! an earlier invocation wrote and the committed bytes never name.
//!
//! Run (writes to the repo's `docs/visuals/`):
//!   cargo run -p animsmith --example gen_docs_visuals
//! Write elsewhere:
//!   cargo run -p animsmith --example gen_docs_visuals -- /some/dir

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out_dir = match std::env::args_os().nth(1) {
        // Reports are rendered from the fixture directory, so a relative
        // output directory is resolved against the caller's own working
        // directory before it is handed to a command that runs elsewhere.
        Some(argument) => std::path::absolute(argument)?,
        None => repository.join(animsmith_testkit::docs_visuals::OUTPUT_DIR),
    };
    let working_dir = repository.join(animsmith_testkit::docs_visuals::WORKING_DIR);
    let animsmith = build_cli(&repository)?;
    // A report whose after side is an AnimSmith command's own output needs
    // somewhere to put that output. It is never committed and never named
    // in a committed byte, so it is a throwaway directory rather than a
    // path anyone has to choose.
    let scratch = tempfile::Builder::new()
        .prefix(animsmith_testkit::docs_visuals::SCRATCH_PREFIX)
        .tempdir()?;

    animsmith_testkit::docs_visuals::write_docs_visuals(&out_dir, scratch.path(), |arguments| {
        let status = Command::new(&animsmith)
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

/// Build the CLI once, and answer where it landed.
///
/// `cargo build --example` never builds the package's binaries, so the
/// CLI has to be built on purpose; but spawning `cargo run` per report
/// paid a freshness check per render — seconds each on a warm cache, for
/// work that takes milliseconds. Cargo puts an example beside the
/// package's binaries — `<target>/<profile>/examples/<example>` next to
/// `<target>/<profile>/<binary>` — so this example's own path names both
/// the target directory and the profile it was built into, and the binary
/// it finds there is the one the same `cargo run` would have used.
fn build_cli(repository: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let example = std::env::current_exe()?;
    let profile_dir = example
        .parent()
        .and_then(Path::parent)
        .ok_or("the example does not sit in <target>/<profile>/examples")?;
    let profile = profile_dir
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("the build profile directory has no name")?;

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut build = Command::new(&cargo);
    build
        .args(["build", "--quiet", "-p", "animsmith"])
        .current_dir(repository);
    // Cargo names the dev profile's directory `debug`; every other
    // profile's directory carries the profile's own name.
    if profile != "debug" {
        build.args(["--profile", profile]);
    }
    let status = build.status()?;
    if !status.success() {
        return Err(format!("cargo build -p animsmith exited with {status}").into());
    }

    let binary = profile_dir.join(format!("animsmith{}", std::env::consts::EXE_SUFFIX));
    if !binary.is_file() {
        return Err(format!("{} was not built", binary.display()).into());
    }
    Ok(binary)
}
