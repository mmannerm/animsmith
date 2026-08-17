//! Emit the rendered CommonMark/GFM structure used by the animation-pack skill.
//!
//! This helper deliberately delegates Markdown syntax to `pulldown-cmark`.
//! The skill's Python orchestration consumes this normalized JSON model and
//! never attempts to recognize headings, tables, links, code, or HTML itself.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Serialize;
use std::io::{self, Read};

#[derive(Clone, Default, Serialize)]
struct RichText {
    text: String,
    code: Vec<String>,
    code_spans: Vec<CodeSpan>,
    strong: Vec<String>,
    links: Vec<RenderedLink>,
}

#[derive(Clone, Serialize)]
struct CodeSpan {
    start: usize,
    end: usize,
    text: String,
}

#[derive(Clone, Serialize)]
struct RenderedLink {
    text: String,
    destination: String,
}

#[derive(Serialize)]
struct Heading {
    level: u8,
    text: String,
    section: String,
    subsection: String,
    blockquote: bool,
    list_depth: usize,
}

#[derive(Serialize)]
struct Paragraph {
    section: String,
    subsection: String,
    text: String,
    code: Vec<String>,
    strong: Vec<String>,
    links: Vec<RenderedLink>,
    blockquote: bool,
    list_item: Option<u64>,
    list_depth: usize,
}

#[derive(Serialize)]
struct Table {
    section: String,
    subsection: String,
    blockquote: bool,
    list_depth: usize,
    header: Vec<RichText>,
    rows: Vec<Vec<RichText>>,
}

#[derive(Serialize)]
struct CodeBlock {
    section: String,
    subsection: String,
    text: String,
    blockquote: bool,
    list_depth: usize,
}

#[derive(Serialize)]
struct RuleBlock {
    section: String,
    subsection: String,
    blockquote: bool,
    list_depth: usize,
}

#[derive(Serialize)]
struct Document {
    first_block: Option<String>,
    headings: Vec<Heading>,
    paragraphs: Vec<Paragraph>,
    tables: Vec<Table>,
    code_blocks: Vec<CodeBlock>,
    rules: Vec<RuleBlock>,
    links: Vec<RenderedLink>,
    placeholders: Vec<String>,
    word_count: usize,
    has_raw_html: bool,
}

#[derive(Default)]
struct TableBuilder {
    section: String,
    subsection: String,
    blockquote: bool,
    list_depth: usize,
    header: Vec<RichText>,
    rows: Vec<Vec<RichText>>,
    row: Vec<RichText>,
    in_head: bool,
}

struct ListState {
    next: Option<u64>,
}

fn append_text(target: &mut RichText, value: &str) {
    target.text.push_str(value);
}

fn append_break(target: &mut RichText) {
    if !target.text.ends_with(' ') && !target.text.is_empty() {
        target.text.push(' ');
    }
}

fn heading_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn scan_placeholders(source: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(start_rel) = source[cursor..].find("{{") {
        let start = cursor + start_rel;
        let Some(end_rel) = source[start + 2..].find("}}") else {
            break;
        };
        let end = start + 2 + end_rel + 2;
        let candidate = &source[start..end];
        if !candidate[2..candidate.len() - 2].contains(['{', '}']) {
            values.push(candidate.to_owned());
        }
        cursor = end;
    }
    values.sort();
    values.dedup();
    values
}

fn count_words(source: &str) -> usize {
    let mut words = 0;
    let mut in_word = false;
    for character in source.chars() {
        let word_character = character.is_alphanumeric()
            || character == '_'
            || character == '\''
            || character == '-';
        if word_character && !in_word {
            words += 1;
        }
        in_word = word_character;
    }
    words
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_GFM);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let mut headings = Vec::new();
    let mut paragraphs = Vec::new();
    let mut tables = Vec::new();
    let mut code_blocks = Vec::new();
    let mut rules = Vec::new();
    let mut links = Vec::new();
    let mut current_section = String::new();
    let mut current_subsection = String::new();
    let mut current_heading: Option<(HeadingLevel, RichText)> = None;
    let mut current_paragraph: Option<RichText> = None;
    let mut current_table: Option<TableBuilder> = None;
    let mut current_cell: Option<RichText> = None;
    let mut blockquote_depth = 0usize;
    let mut list_stack: Vec<ListState> = Vec::new();
    let mut current_list_item = None;
    let mut current_item_text: Option<RichText> = None;
    let mut item_had_paragraph = false;
    let mut code_block_depth = 0usize;
    let mut current_code_block: Option<CodeBlock> = None;
    let mut html_block_depth = 0usize;
    let mut has_raw_html = false;
    let mut strong_depth = 0usize;
    let mut strong_text = String::new();
    let mut current_link: Option<(String, String)> = None;
    let mut first_block = None;

    for event in Parser::new_ext(&source, options) {
        if first_block.is_none() {
            first_block = match &event {
                Event::Start(Tag::Heading { level, .. }) => {
                    Some(format!("heading:{}", heading_number(*level)))
                }
                Event::Start(Tag::Paragraph) => Some("paragraph".to_owned()),
                Event::Start(Tag::BlockQuote(_)) => Some("blockquote".to_owned()),
                Event::Start(Tag::CodeBlock(_)) => Some("code-block".to_owned()),
                Event::Start(Tag::List(_)) => Some("list".to_owned()),
                Event::Start(Tag::Table(_)) => Some("table".to_owned()),
                Event::Rule => Some("rule".to_owned()),
                _ => None,
            };
        }
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                code_block_depth += 1;
                if code_block_depth == 1 {
                    current_code_block = Some(CodeBlock {
                        section: current_section.clone(),
                        subsection: current_subsection.clone(),
                        text: String::new(),
                        blockquote: blockquote_depth > 0,
                        list_depth: list_stack.len(),
                    });
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if code_block_depth == 1
                    && let Some(block) = current_code_block.take()
                {
                    code_blocks.push(block);
                }
                code_block_depth = code_block_depth.saturating_sub(1);
            }
            Event::Start(Tag::HtmlBlock) => {
                html_block_depth += 1;
                has_raw_html = true;
            }
            Event::End(TagEnd::HtmlBlock) => {
                html_block_depth = html_block_depth.saturating_sub(1);
            }
            Event::Text(value) if code_block_depth > 0 => {
                if let Some(block) = current_code_block.as_mut() {
                    block.text.push_str(&value);
                }
            }
            _ if code_block_depth > 0 || html_block_depth > 0 => {}
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some((level, RichText::default()));
            }
            Event::End(TagEnd::Heading(level)) => {
                if let Some((_started_level, rich)) = current_heading.take() {
                    let text = rich.text.trim().to_owned();
                    let parent_section = current_section.clone();
                    let parent_subsection = current_subsection.clone();
                    let top_level = blockquote_depth == 0 && list_stack.is_empty();
                    if level == HeadingLevel::H2 && top_level {
                        current_section.clone_from(&text);
                        current_subsection.clear();
                    } else if level == HeadingLevel::H3 && top_level {
                        current_subsection.clone_from(&text);
                    }
                    headings.push(Heading {
                        level: heading_number(level),
                        section: if level == HeadingLevel::H2 && top_level {
                            text.clone()
                        } else {
                            parent_section
                        },
                        subsection: if level == HeadingLevel::H3 && top_level {
                            text.clone()
                        } else {
                            parent_subsection
                        },
                        text,
                        blockquote: blockquote_depth > 0,
                        list_depth: list_stack.len(),
                    });
                }
            }
            Event::Start(Tag::Paragraph) => current_paragraph = Some(RichText::default()),
            Event::End(TagEnd::Paragraph) => {
                if let Some(rich) = current_paragraph.take() {
                    if current_list_item.is_some() {
                        item_had_paragraph = true;
                    }
                    paragraphs.push(Paragraph {
                        section: current_section.clone(),
                        subsection: current_subsection.clone(),
                        text: rich.text.trim().to_owned(),
                        code: rich.code,
                        strong: rich.strong,
                        links: rich.links,
                        blockquote: blockquote_depth > 0,
                        list_item: current_list_item,
                        list_depth: list_stack.len(),
                    });
                }
            }
            Event::Start(Tag::BlockQuote(_)) => blockquote_depth += 1,
            Event::End(TagEnd::BlockQuote(_)) => {
                blockquote_depth = blockquote_depth.saturating_sub(1)
            }
            Event::Start(Tag::List(start)) => list_stack.push(ListState { next: start }),
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
                current_list_item = None;
            }
            Event::Start(Tag::Item) => {
                current_list_item = list_stack.last_mut().and_then(|state| {
                    let value = state.next?;
                    state.next = Some(value + 1);
                    Some(value)
                });
                current_item_text = Some(RichText::default());
                item_had_paragraph = false;
            }
            Event::End(TagEnd::Item) => {
                if !item_had_paragraph {
                    if let Some(rich) = current_item_text.take() {
                        paragraphs.push(Paragraph {
                            section: current_section.clone(),
                            subsection: current_subsection.clone(),
                            text: rich.text.trim().to_owned(),
                            code: rich.code,
                            strong: rich.strong,
                            links: rich.links,
                            blockquote: blockquote_depth > 0,
                            list_item: current_list_item,
                            list_depth: list_stack.len(),
                        });
                    }
                } else {
                    current_item_text = None;
                }
                current_list_item = None;
            }
            Event::Start(Tag::Table(_)) => {
                current_table = Some(TableBuilder {
                    section: current_section.clone(),
                    subsection: current_subsection.clone(),
                    blockquote: blockquote_depth > 0,
                    list_depth: list_stack.len(),
                    ..TableBuilder::default()
                });
            }
            Event::End(TagEnd::Table) => {
                if let Some(table) = current_table.take() {
                    tables.push(Table {
                        section: table.section,
                        subsection: table.subsection,
                        blockquote: table.blockquote,
                        list_depth: table.list_depth,
                        header: table.header,
                        rows: table.rows,
                    });
                }
            }
            Event::Start(Tag::TableHead) => {
                if let Some(table) = current_table.as_mut() {
                    table.in_head = true;
                    table.row.clear();
                }
            }
            Event::End(TagEnd::TableHead) => {
                if let Some(table) = current_table.as_mut() {
                    table.in_head = false;
                    table.header = std::mem::take(&mut table.row);
                }
            }
            Event::Start(Tag::TableRow) => {
                if let Some(table) = current_table.as_mut() {
                    table.row.clear();
                }
            }
            Event::End(TagEnd::TableRow) => {
                if let Some(table) = current_table.as_mut() {
                    let row = std::mem::take(&mut table.row);
                    if table.in_head {
                        table.header = row;
                    } else {
                        table.rows.push(row);
                    }
                }
            }
            Event::Start(Tag::TableCell) => current_cell = Some(RichText::default()),
            Event::End(TagEnd::TableCell) => {
                if let (Some(cell), Some(table)) = (current_cell.take(), current_table.as_mut()) {
                    table.row.push(cell);
                }
            }
            Event::Rule => rules.push(RuleBlock {
                section: current_section.clone(),
                subsection: current_subsection.clone(),
                blockquote: blockquote_depth > 0,
                list_depth: list_stack.len(),
            }),
            Event::Start(Tag::Strong) => {
                strong_depth += 1;
                if strong_depth == 1 {
                    strong_text.clear();
                }
            }
            Event::End(TagEnd::Strong) => {
                if strong_depth == 1 {
                    let value = strong_text.trim().to_owned();
                    if !value.is_empty() {
                        if let Some(cell) = current_cell.as_mut() {
                            cell.strong.push(value);
                        } else if let Some(paragraph) = current_paragraph.as_mut() {
                            paragraph.strong.push(value);
                        } else if let Some((_level, heading)) = current_heading.as_mut() {
                            heading.strong.push(value);
                        } else if let Some(item) = current_item_text.as_mut() {
                            item.strong.push(value);
                        }
                    }
                }
                strong_depth = strong_depth.saturating_sub(1);
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                current_link = Some((dest_url.into_string(), String::new()));
            }
            Event::End(TagEnd::Link) => {
                if let Some((destination, text)) = current_link.take() {
                    let link = RenderedLink { text, destination };
                    if let Some(cell) = current_cell.as_mut() {
                        cell.links.push(link.clone());
                    } else if let Some(paragraph) = current_paragraph.as_mut() {
                        paragraph.links.push(link.clone());
                    } else if let Some((_level, heading)) = current_heading.as_mut() {
                        heading.links.push(link.clone());
                    } else if let Some(item) = current_item_text.as_mut() {
                        item.links.push(link.clone());
                    }
                    links.push(link);
                }
            }
            event @ (Event::Text(_) | Event::Code(_)) => {
                let (value, is_code) = match &event {
                    Event::Text(value) => (value.as_ref(), false),
                    Event::Code(value) => (value.as_ref(), true),
                    _ => unreachable!(),
                };
                if let Some(cell) = current_cell.as_mut() {
                    let start = cell.text.chars().count();
                    append_text(cell, value);
                    if is_code {
                        cell.code.push(value.to_owned());
                        cell.code_spans.push(CodeSpan {
                            start,
                            end: cell.text.chars().count(),
                            text: value.to_owned(),
                        });
                    }
                } else if let Some(paragraph) = current_paragraph.as_mut() {
                    let start = paragraph.text.chars().count();
                    append_text(paragraph, value);
                    if is_code {
                        paragraph.code.push(value.to_owned());
                        paragraph.code_spans.push(CodeSpan {
                            start,
                            end: paragraph.text.chars().count(),
                            text: value.to_owned(),
                        });
                    }
                } else if let Some((_level, heading)) = current_heading.as_mut() {
                    let start = heading.text.chars().count();
                    append_text(heading, value);
                    if is_code {
                        heading.code.push(value.to_owned());
                        heading.code_spans.push(CodeSpan {
                            start,
                            end: heading.text.chars().count(),
                            text: value.to_owned(),
                        });
                    }
                } else if let Some(item) = current_item_text.as_mut() {
                    let start = item.text.chars().count();
                    append_text(item, value);
                    if is_code {
                        item.code.push(value.to_owned());
                        item.code_spans.push(CodeSpan {
                            start,
                            end: item.text.chars().count(),
                            text: value.to_owned(),
                        });
                    }
                }
                if strong_depth > 0 {
                    strong_text.push_str(value);
                }
                if let Some((_destination, link_text)) = current_link.as_mut() {
                    link_text.push_str(value);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(cell) = current_cell.as_mut() {
                    append_break(cell);
                } else if let Some(paragraph) = current_paragraph.as_mut() {
                    append_break(paragraph);
                } else if let Some((_level, heading)) = current_heading.as_mut() {
                    append_break(heading);
                } else if let Some(item) = current_item_text.as_mut() {
                    append_break(item);
                }
            }
            Event::InlineHtml(_) => has_raw_html = true,
            _ => {}
        }
    }

    let rendered_words = paragraphs
        .iter()
        .map(|paragraph| count_words(&paragraph.text))
        .sum::<usize>()
        + headings
            .iter()
            .map(|heading| count_words(&heading.text))
            .sum::<usize>()
        + tables
            .iter()
            .flat_map(|table| table.header.iter().chain(table.rows.iter().flatten()))
            .map(|cell| count_words(&cell.text))
            .sum::<usize>()
        + code_blocks
            .iter()
            .map(|block| count_words(&block.text))
            .sum::<usize>();
    let document = Document {
        first_block,
        headings,
        paragraphs,
        tables,
        code_blocks,
        rules,
        links,
        placeholders: scan_placeholders(&source),
        word_count: rendered_words,
        has_raw_html,
    };
    serde_json::to_writer(io::stdout(), &document)?;
    Ok(())
}
