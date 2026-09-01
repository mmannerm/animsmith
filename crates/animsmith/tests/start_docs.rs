//! Drift guard for the Start pages and the symptom pages: every
//! `$ animsmith …` command they document is run against the committed
//! example fixtures, and its exit code and quoted output must still be
//! what the page claims.
//!
//! The pages are customer-facing, so a stale transcript is a broken
//! promise rather than a cosmetic problem. Each `console` block is
//! parsed the way a reader reads it: a `$ ` line (with `\` continuations)
//! is the command, the lines under it are its output, and the trailing
//! `# exits N` marker is the exit code the page claims. A quoted line
//! ending in `...` is matched as a prefix — the cookbook's convention for
//! trimming one long line — and every other quoted line must appear
//! verbatim.
//!
//! Commands run in a temporary copy of `examples/`, so a documented
//! `-o fixed.glb` writes where the reader's own checkout would put it
//! without touching this one.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The customer pages whose transcripts this gate pins.
const PAGES: &[&str] = &[
    "docs/install.md",
    "docs/first-lint.md",
    "docs/first-report.md",
    "docs/symptoms/loop-pops.md",
    "docs/symptoms/feet-slide.md",
];

/// A trimmed line: the reader is told the rest was cut, so only the
/// prefix is promised.
const TRIM: &str = "...";

/// The exit-code claim every documented `animsmith` command must carry.
const EXITS: &str = "# exits ";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
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

/// Every `console` block of one page, in document order. Only rendered
/// fenced blocks count, so an indented or differently tagged sample is
/// not mistaken for a promise.
fn console_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for event in Parser::new_ext(markdown, Options::empty()) {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language)))
                if language.as_ref() == "console" =>
            {
                current = Some(String::new());
            }
            Event::Text(text) => {
                if let Some(block) = current.as_mut() {
                    block.push_str(&text);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(block) = current.take() {
                    blocks.push(block);
                }
            }
            _ => {}
        }
    }
    blocks
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
        let last = segment.pop().expect("a segment starts with its command");
        let (last, exit) = match last.split_once(EXITS) {
            Some((text, code)) => (
                text.trim_end(),
                Some(code.trim().parse().unwrap_or_else(|error| {
                    panic!("{page}: unreadable exit claim {last:?}: {error}")
                })),
            ),
            None => (last, None),
        };
        segment.push(last);

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
        commands.push(Documented {
            command: command.trim().trim_start_matches("$ ").trim().to_owned(),
            output: lines.map(str::to_owned).collect(),
            exit,
        });
    }
    commands
}

/// A temporary checkout-shaped directory: the committed `examples/` tree
/// and nothing else, so a documented command's relative paths resolve and
/// its outputs land outside this repository.
fn fixture_checkout() -> tempfile::TempDir {
    let temporary = tempfile::Builder::new()
        .prefix("animsmith-start-docs-")
        .tempdir()
        .expect("creates temp dir");
    copy_tree(&repo_root().join("examples"), &temporary.path().join("examples"));
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
    for page in PAGES {
        let markdown = std::fs::read_to_string(repo_root().join(page))
            .unwrap_or_else(|error| panic!("reads {page}: {error}"));
        let checkout = fixture_checkout();
        let mut ran = 0usize;

        for block in console_blocks(&markdown) {
            for documented in documented_commands(&block, page) {
                assert!(
                    !documented.command.contains(['"', '\'']),
                    "{page}: documented commands are quote-free by construction: {}",
                    documented.command
                );
                let Some(arguments) = documented.command.strip_prefix("animsmith ") else {
                    continue;
                };
                let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
                    .args(arguments.split_whitespace())
                    .current_dir(checkout.path())
                    .output()
                    .unwrap_or_else(|error| panic!("{page}: runs {arguments}: {error}"));
                let stdout = String::from_utf8_lossy(&output.stdout);

                let claimed = documented.exit.unwrap_or_else(|| {
                    panic!("{page}: `animsmith {arguments}` has no `{EXITS}N` claim")
                });
                assert_eq!(
                    output.status.code(),
                    Some(claimed),
                    "{page}: `animsmith {arguments}` exit code changed\nstdout:\n{stdout}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                for line in &documented.output {
                    let matched = match line.strip_suffix(TRIM) {
                        Some(prefix) => stdout.lines().any(|actual| actual.starts_with(prefix)),
                        None => stdout.lines().any(|actual| actual == line),
                    };
                    assert!(
                        matched,
                        "{page}: `animsmith {arguments}` no longer prints {line:?}\nstdout:\n{stdout}"
                    );
                }
                ran += 1;
                total += 1;
            }
        }
        assert!(ran > 0, "{page} documents no runnable animsmith command");
    }
    assert!(
        total >= 10,
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
         $ animsmith --version   # exits 2\n";
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
        ]
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

/// Only fenced `console` blocks are promises: a differently tagged block
/// is illustration, and an indented one is not a rendered fence at all.
#[test]
fn only_console_fences_are_read_as_promises() {
    let markdown = "```console\n$ animsmith lint clip.glb   # exits 0\n```\n\n\
         ```toml\n[clips.walk]\nloop = true\n```\n\n\
         \x20   $ animsmith lint indented.glb   # exits 0\n";
    assert_eq!(
        console_blocks(markdown),
        ["$ animsmith lint clip.glb   # exits 0\n"]
    );
}
