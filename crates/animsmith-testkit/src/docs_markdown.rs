//! Readers for the Markdown the documentation gates hold to its word.
//!
//! Every gate reads rendered Markdown rather than source lines, so a
//! promise cannot be satisfied by a code-shaped decoy and cannot be
//! broken by ordinary Markdown syntax. The two readers several gates
//! share live here so they cannot disagree about what a page says.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::collections::BTreeSet;

/// GitHub-flavored parser options, so what the gates see is what
/// github.com renders. Tables carry the index rows; the rest keeps the
/// gates from diverging over an enabled extension.
fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_TASKLISTS
}

/// The body of every fenced code block tagged `language`, in document
/// order.
///
/// Only rendered fences count: an indented block or a differently tagged
/// one is illustration, not a promise the transcript gates run.
pub fn fenced_blocks(markdown: &str, language: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info)))
                if info.as_ref() == language =>
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

/// Link destinations, `#fragment`s stripped, in the **Document column** —
/// the first cell of each body row — of the first table whose leading
/// header cell is exactly `Document`.
///
/// This is the canonical index shape: `docs/README.md` owns `docs/`, and
/// `docs/symptoms/README.md` owns its directory the same way. A link in
/// a description cell is not a row for that page, and text that merely
/// looks like a link never produces a link event.
pub fn document_index_targets(markdown: &str) -> BTreeSet<String> {
    let mut targets = BTreeSet::new();
    let mut in_head = false;
    let mut head_first_cell: Option<String> = None;
    let mut collecting_first_cell = false;
    let mut is_index_table = false;
    let mut body_cell_index = 0usize;

    for event in Parser::new_ext(markdown, options()) {
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
            Event::Start(Tag::TableRow) if is_index_table => body_cell_index = 0,
            Event::Start(Tag::TableCell) if is_index_table && !in_head => {
                body_cell_index += 1;
            }
            Event::Start(Tag::Link { dest_url, .. })
                if is_index_table && !in_head && body_cell_index == 1 =>
            {
                targets.insert(dest_url.split('#').next().unwrap_or_default().to_owned());
            }
            Event::End(TagEnd::Table) if is_index_table => break,
            _ => {}
        }
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only fenced blocks with the asked-for tag are promises: a
    /// differently tagged block is illustration, and an indented one is
    /// not a rendered fence at all.
    #[test]
    fn only_fences_with_the_named_language_are_read() {
        let markdown = "```console\n$ animsmith lint clip.glb   # exits 0\n```\n\n\
             ```toml\n[clips.walk]\nloop = true\n```\n\n\
             \x20   $ animsmith lint indented.glb   # exits 0\n\n\
             ```console\n$ animsmith report clip.glb   # exits 0\n```\n";
        assert_eq!(
            fenced_blocks(markdown, "console"),
            [
                "$ animsmith lint clip.glb   # exits 0\n",
                "$ animsmith report clip.glb   # exits 0\n",
            ]
        );
        assert_eq!(
            fenced_blocks(markdown, "toml"),
            ["[clips.walk]\nloop = true\n"]
        );
    }

    /// The mutation catalog the shell oracle accumulated across audit
    /// rounds, pinned against the parser: only rendered links inside the
    /// Document table count.
    #[test]
    fn only_rendered_links_inside_the_document_table_are_index_rows() {
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
            | [real.md](real.md) | see also [wrong-column.md](wrong-column.md), which has no row |\n\
            | malformed ](broken.md) cell without an opening bracket |\n\n\
            Prose with a loose [prose.md](prose.md) link.\n\n\
            | standalone pipe line with [loose.md](loose.md) outside any table |\n";

        let expected: BTreeSet<String> = ["real.md", "fragment.md"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(
            document_index_targets(fixture),
            expected,
            "only rendered links in the Document column count"
        );
    }
}
