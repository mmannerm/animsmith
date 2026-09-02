//! One reader for the console transcripts the documentation pages print.
//!
//! A page's `console` block is parsed the way a reader reads it: a `$ `
//! line (with `\` continuations) is the command, the lines under it are
//! its output, and the `# exits N` marker — on the command line for a
//! silent command, or at the end of the last output line — is the exit
//! code the page claims. A quoted line ending in `...` is matched as a
//! prefix, which is the cookbook's convention for trimming one long line.
//!
//! Both the Start-page gate and the cookbook gate read transcripts this
//! way, so this lives here rather than in either of them: a page that goes
//! stale has to fail in whichever gate covers it, and two parsers would
//! eventually disagree about what a page promised.

/// A trimmed line: the reader is told the rest was cut, so only the
/// prefix is promised.
pub const TRIM: &str = "...";

/// The exit-code claim every documented `animsmith` command must carry.
pub const EXITS: &str = "# exits ";

/// One documented command: what it runs, what it prints, and what it
/// claims to return. Only `animsmith` commands must carry a claim; the
/// pages also show `cargo` and shell lines this gate does not run.
#[derive(Debug, PartialEq, Eq)]
pub struct Documented {
    /// The command the page shows, without its `$ ` prompt.
    pub command: String,
    /// The lines the page quotes under it.
    pub output: Vec<String>,
    /// The exit code the page claims, if it makes a claim.
    pub exit: Option<i32>,
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
pub fn documented_commands(block: &str, page: &str) -> Vec<Documented> {
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
pub fn documents(line: &str, actual: &str) -> bool {
    match line.strip_suffix(TRIM) {
        Some(prefix) => actual.starts_with(prefix),
        None => actual == line,
    }
}

/// The first documented line the output no longer shows where the page
/// shows it. A transcript is read top to bottom, so each quoted line must
/// match a printed line below the previous one's match: a page may quote
/// part of the output, but not a rearrangement of it.
pub fn misdocumented_line(documented: &[String], printed: &[&str]) -> Option<usize> {
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
