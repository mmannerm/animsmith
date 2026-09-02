//! Drift guard for the Start pages and the symptom pages: every
//! `$ animsmith …` command they document is run against the committed
//! example fixtures, and its exit code and quoted output must still be
//! what the page claims.
//!
//! The pages are customer-facing, so a stale transcript is a broken
//! promise rather than a cosmetic problem. Each `console` block is
//! parsed the way a reader reads it: a `$ ` line (with `\` continuations)
//! is the command, the lines under it are its output, and the
//! `# exits N` marker — on the command line for a silent command, or at
//! the end of the last output line — is the exit code the page claims.
//! A quoted line ending in `...` is matched as a prefix — the cookbook's
//! convention for trimming one long line — and every other quoted line
//! must appear verbatim. The quoted lines must also appear in the order
//! the page prints them, so a reader following along sees what the page
//! shows rather than the same lines rearranged.
//!
//! Commands run in a temporary copy of `examples/`, so a documented
//! `-o fixed.glb` writes where the reader's own checkout would put it
//! without touching this one.

use animsmith_testkit::docs_markdown::fenced_blocks;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The Start pages whose transcripts this gate pins. Every symptom page
/// joins them automatically, so writing one gates it.
const START_PAGES: &[&str] = &["docs/install.md", "docs/first-lint.md"];

/// `docs/first-report.md` documents the `report` command, which only the
/// `report` feature builds; a `--no-default-features` binary would fail
/// every line of it, so the page is gated exactly when the feature is on.
#[cfg(feature = "report")]
const REPORT_PAGES: &[&str] = &["docs/first-report.md"];
#[cfg(not(feature = "report"))]
const REPORT_PAGES: &[&str] = &[];

/// The directory whose pages are gated as a whole.
const SYMPTOMS_DIR: &str = "docs/symptoms";

/// Committed trees the documented commands name, copied into every
/// page's throwaway checkout so a relative path in a transcript resolves
/// exactly as it does in a reader's own checkout.
const FIXTURE_TREES: &[&str] = &["examples", "crates/animsmith/testdata/collection-spike"];

/// A trimmed line: the reader is told the rest was cut, so only the
/// prefix is promised.
const TRIM: &str = "...";

/// The exit-code claim every documented `animsmith` command must carry.
const EXITS: &str = "# exits ";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every gated page: the Start pages plus each page of the symptom
/// directory, read from disk rather than from a hand-maintained list, so
/// a new symptom page is gated the moment it is committed.
fn gated_pages() -> Vec<String> {
    let mut symptoms: Vec<String> = std::fs::read_dir(repo_root().join(SYMPTOMS_DIR))
        .expect("lists the symptom pages")
        .filter_map(|entry| {
            let name = entry.expect("directory entry").file_name();
            let name = name.into_string().expect("utf-8 page name");
            (name.ends_with(".md") && name != "README.md").then(|| format!("{SYMPTOMS_DIR}/{name}"))
        })
        .collect();
    symptoms.sort();
    assert!(
        !symptoms.is_empty(),
        "{SYMPTOMS_DIR}/ must publish the symptom pages this gate runs"
    );

    let mut pages: Vec<String> = START_PAGES
        .iter()
        .chain(REPORT_PAGES)
        .map(|page| (*page).to_owned())
        .collect();
    pages.extend(symptoms);
    pages
}

/// One documented command: what it runs, what it prints, and what it
/// claims to return. Only `animsmith` commands must carry a claim; the
/// pages also show `cargo` and shell lines this gate does not run.
#[derive(Debug, PartialEq, Eq)]
struct Documented {
    command: String,
    output: Vec<String>,
    exit: Option<i32>,
}

/// Take the one `# exits N` claim out of a segment, wherever the page
/// wrote it: at the end of the command line for a command that prints
/// nothing, or at the end of its last output line. The marker is removed
/// from the line it sat on, so it never reaches the binary as an
/// argument, and a segment that claims twice is ambiguous rather than
/// silently resolved.
fn take_exit_claim(segment: &mut [&str], page: &str) -> Option<i32> {
    let mut claim = None;
    for line in segment.iter_mut() {
        let original = *line;
        let Some((text, code)) = original.split_once(EXITS) else {
            continue;
        };
        let code = code
            .trim()
            .parse()
            .unwrap_or_else(|error| panic!("{page}: unreadable exit claim {original:?}: {error}"));
        assert!(
            claim.is_none(),
            "{page}: one `{EXITS}N` claim per command, found another in {original:?}"
        );
        claim = Some(code);
        *line = text.trim_end();
    }
    claim
}

/// Split one block into its documented commands.
fn documented_commands(block: &str, page: &str) -> Vec<Documented> {
    let mut commands = Vec::new();
    let mut segments: Vec<Vec<&str>> = Vec::new();
    for line in block.lines() {
        if line.starts_with("$ ") {
            segments.push(vec![line]);
        } else if let Some(segment) = segments.last_mut() {
            segment.push(line);
        }
    }

    for mut segment in segments {
        while segment.last().is_some_and(|line| line.trim().is_empty()) {
            segment.pop();
        }
        let exit = take_exit_claim(&mut segment, page);

        // A `\` continues the command onto the next line; everything after
        // the command's last line is its output.
        let mut command = String::new();
        let mut lines = segment.into_iter();
        for line in lines.by_ref() {
            let line = line.trim();
            command.push(' ');
            match line.strip_suffix('\\') {
                Some(head) => command.push_str(head.trim_end()),
                None => {
                    command.push_str(line);
                    break;
                }
            }
        }
        let output = lines
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                assert!(
                    line.strip_suffix(TRIM)
                        .is_none_or(|prefix| !prefix.trim().is_empty()),
                    "{page}: a trimmed line must promise something before its `{TRIM}`: {line:?}"
                );
                line.to_owned()
            })
            .collect();
        commands.push(Documented {
            command: command.trim().trim_start_matches("$ ").trim().to_owned(),
            output,
            exit,
        });
    }
    commands
}

/// Whether one documented line still describes an actual output line. A
/// trailing `...` promises only the prefix before it.
fn documents(line: &str, actual: &str) -> bool {
    match line.strip_suffix(TRIM) {
        Some(prefix) => actual.starts_with(prefix),
        None => actual == line,
    }
}

/// The first documented line the output no longer shows where the page
/// shows it. A transcript is read top to bottom, so each quoted line must
/// match a printed line below the previous one's match: a page may quote
/// part of the output, but not a rearrangement of it.
fn misdocumented_line(documented: &[String], printed: &[&str]) -> Option<usize> {
    let mut next = 0usize;
    for (index, line) in documented.iter().enumerate() {
        match printed[next..]
            .iter()
            .position(|actual| documents(line, actual))
        {
            Some(offset) => next += offset + 1,
            None => return Some(index),
        }
    }
    None
}

/// A temporary checkout-shaped directory: the committed [`FIXTURE_TREES`]
/// and nothing else, so a documented command's relative paths resolve and
/// its outputs land outside this repository.
fn fixture_checkout() -> tempfile::TempDir {
    let temporary = tempfile::Builder::new()
        .prefix("animsmith-start-docs-")
        .tempdir()
        .expect("creates temp dir");
    for tree in FIXTURE_TREES {
        copy_tree(&repo_root().join(tree), &temporary.path().join(tree));
    }
    temporary
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("creates fixture directory");
    for entry in std::fs::read_dir(source).expect("lists fixture directory") {
        let entry = entry.expect("directory entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copies fixture");
        }
    }
}

#[test]
fn every_documented_start_command_still_behaves_as_the_page_claims() {
    let mut total = 0usize;
    let pages = gated_pages();
    for page in &pages {
        let markdown = std::fs::read_to_string(repo_root().join(page))
            .unwrap_or_else(|error| panic!("reads {page}: {error}"));
        let checkout = fixture_checkout();
        let mut ran = 0usize;

        for block in fenced_blocks(&markdown, "console") {
            for documented in documented_commands(&block, page) {
                assert!(
                    !documented.command.contains(['"', '\'']),
                    "{page}: documented commands are quote-free by construction: {}",
                    documented.command
                );
                let Some(arguments) = documented.command.strip_prefix("animsmith ") else {
                    continue;
                };
                // The claim is read before the command runs: a page that
                // documents no exit code documents nothing runnable.
                let claimed = documented.exit.unwrap_or_else(|| {
                    panic!("{page}: `animsmith {arguments}` has no `{EXITS}N` claim")
                });
                let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
                    .args(arguments.split_whitespace())
                    .current_dir(checkout.path())
                    .output()
                    .unwrap_or_else(|error| panic!("{page}: runs {arguments}: {error}"));
                let stdout = String::from_utf8_lossy(&output.stdout);

                assert_eq!(
                    output.status.code(),
                    Some(claimed),
                    "{page}: `animsmith {arguments}` exit code changed\nstdout:\n{stdout}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let printed: Vec<&str> = stdout.lines().collect();
                if let Some(index) = misdocumented_line(&documented.output, &printed) {
                    let line = &documented.output[index];
                    panic!(
                        "{page}: `animsmith {arguments}` no longer prints {line:?} where the \
                         page documents it, below the lines quoted before it\nstdout:\n{stdout}"
                    );
                }
                ran += 1;
                total += 1;
            }
        }
        assert!(ran > 0, "{page} documents no runnable animsmith command");
    }
    assert!(
        total >= 25,
        "the Start and symptom pages must keep documenting their commands, found {total}"
    );
}

/// The parser reads a block the way a reader does: continuations join,
/// output follows, and an exit claim is recorded where the page makes one.
#[test]
fn the_transcript_parser_reads_commands_output_and_exit_claims() {
    let block = "$ animsmith lint clip.glb\nclip.glb:\n\
         0 error(s)   # exits 0\n\n\
         $ animsmith report clip.glb \\\n    -o report.html\n\
         wrote report.html ...   # exits 0\n\
         $ animsmith --version   # exits 2\n\
         $ animsmith lint broken.glb   # exits 1\n\
         broken.glb:\n\
        \x20 error[nan] clip 'swing' ...\n";
    assert_eq!(
        documented_commands(block, "fixture.md"),
        [
            Documented {
                command: "animsmith lint clip.glb".to_owned(),
                output: vec!["clip.glb:".to_owned(), "0 error(s)".to_owned()],
                exit: Some(0),
            },
            Documented {
                command: "animsmith report clip.glb -o report.html".to_owned(),
                output: vec!["wrote report.html ...".to_owned()],
                exit: Some(0),
            },
            Documented {
                command: "animsmith --version".to_owned(),
                output: Vec::new(),
                exit: Some(2),
            },
            Documented {
                command: "animsmith lint broken.glb".to_owned(),
                output: vec![
                    "broken.glb:".to_owned(),
                    "  error[nan] clip 'swing' ...".to_owned(),
                ],
                exit: Some(1),
            },
        ],
        "an exit claim is read wherever the page writes it, and never reaches the binary"
    );

    assert_eq!(
        documented_commands("$ cargo install animsmith\n", "fixture.md"),
        [Documented {
            command: "cargo install animsmith".to_owned(),
            output: Vec::new(),
            exit: None,
        }],
        "a non-animsmith line is read but claims nothing"
    );
}

/// A page that claims two exit codes for one command claims nothing: the
/// reader cannot tell which the tool returns, so the parser refuses it
/// rather than picking one.
#[test]
#[should_panic(expected = "one `# exits N` claim per command")]
fn a_command_may_not_claim_two_exit_codes() {
    documented_commands(
        "$ animsmith lint clip.glb   # exits 0\nclip.glb:   # exits 1\n",
        "fixture.md",
    );
}

/// `...` says the rest of a long line was cut, so something must come
/// before it; a bare `...` would match every line the tool ever prints.
#[test]
#[should_panic(expected = "must promise something before its `...`")]
fn a_trimmed_line_must_promise_a_prefix() {
    documented_commands(
        "$ animsmith lint clip.glb\n...\n0 error(s)   # exits 0\n",
        "fixture.md",
    );
}

/// Quoting part of the output is fine; quoting it in another order is
/// not, because a reader follows the transcript top to bottom.
#[test]
fn quoted_lines_must_appear_in_the_order_the_page_shows_them() {
    let documented = ["clip.glb:".to_owned(), "  error[nan] ...".to_owned()];
    assert_eq!(
        misdocumented_line(
            &documented,
            &["clip.glb:", "  error[nan] clip 'swing'", "1 error(s)"]
        ),
        None,
        "quoting a prefix of two of three printed lines is what a page does"
    );
    assert_eq!(
        misdocumented_line(&documented, &["  error[nan] clip 'swing'", "clip.glb:"]),
        Some(1),
        "the same lines in the other order no longer match the page"
    );
    assert_eq!(
        misdocumented_line(&documented, &["clip.glb:"]),
        Some(1),
        "a line the tool stopped printing is named"
    );
    assert_eq!(
        misdocumented_line(&documented, &["0 error(s)"]),
        Some(0),
        "the first missing line is the one reported"
    );

    let twice = ["same".to_owned(), "same".to_owned()];
    assert_eq!(misdocumented_line(&twice, &["same", "same"]), None);
    assert_eq!(
        misdocumented_line(&twice, &["same"]),
        Some(1),
        "two documented lines need two printed ones"
    );
}
