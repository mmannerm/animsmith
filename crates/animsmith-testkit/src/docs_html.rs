//! One scanner for the raw HTML the documentation embeds.
//!
//! The customer pages carry raw HTML that Markdown cannot express — the
//! generated charts and the embedded reports — and the site's front door
//! is raw HTML from top to bottom. Several gates need the same question
//! answered, "what does this page ask the browser to follow or fetch?",
//! so they ask it here instead of each rolling a `find("<img")` loop.
//! Hand-rolled copies drifted: one accepted single quotes and the other
//! did not, one stopped at the first `src`-shaped attribute so a
//! preceding `data-src` hid the real one, and both matched tag and
//! attribute names case-sensitively.
//!
//! This reads start tags the way a browser's tokenizer does for the part
//! that matters here: tag and attribute names are ASCII-case-insensitive
//! and end at a real boundary (`<image>` is not `<img>`, `data-src` is
//! not `src`), values may be double-quoted, single-quoted or unquoted,
//! comments hide nothing, and references come back in document order
//! across tag kinds. Values are taken literally: a character reference
//! in a local path would be a bug in the page rather than a target a
//! gate should resolve.

/// Every requested `(tag, attribute value)` reference, in document order.
///
/// `wanted` names the `(tag, attribute)` pairs to collect, such as
/// `[("img", "src"), ("iframe", "src")]`. Names are matched ignoring
/// ASCII case and the returned tag name is lowercased. A tag that
/// repeats an attribute yields its first value, as a browser does.
pub fn html_references(html: &str, wanted: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut references = Vec::new();
    for tag in start_tags(html) {
        for (name, attribute) in wanted {
            if tag.name.eq_ignore_ascii_case(name)
                && let Some(value) = tag.attribute(attribute)
            {
                references.push((tag.name.clone(), value.to_owned()));
            }
        }
    }
    references
}

/// One raw-HTML start tag: its lowercased name and its attributes in
/// document order, names lowercased and values unquoted.
struct StartTag {
    name: String,
    attributes: Vec<(String, String)>,
}

impl StartTag {
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(attribute, _)| attribute.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Whether `byte` continues an HTML tag or attribute name.
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
}

/// Every start tag in `html`, in document order.
fn start_tags(html: &str) -> Vec<StartTag> {
    let bytes = html.as_bytes();
    let mut tags = Vec::new();
    let mut index = 0;

    while let Some(offset) = html[index..].find('<') {
        let mut cursor = index + offset + 1;
        // A comment hides everything up to its terminator, including a
        // tag-shaped decoy.
        if html[cursor..].starts_with("!--") {
            index = match html[cursor..].find("-->") {
                Some(end) => cursor + end + "-->".len(),
                None => html.len(),
            };
            continue;
        }
        // A start tag's name begins with an ASCII letter, so `</`, `<!`
        // and a bare `<` in prose open nothing.
        if !bytes.get(cursor).is_some_and(u8::is_ascii_alphabetic) {
            index = cursor;
            continue;
        }

        let name_start = cursor;
        while bytes.get(cursor).is_some_and(|byte| is_name_byte(*byte)) {
            cursor += 1;
        }
        let name = html[name_start..cursor].to_ascii_lowercase();
        let mut attributes = Vec::new();

        loop {
            while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
                cursor += 1;
            }
            match bytes.get(cursor) {
                None => break,
                Some(b'>') => {
                    cursor += 1;
                    break;
                }
                // The `/` of a self-closing tag, or a stray one.
                Some(b'/') => {
                    cursor += 1;
                    continue;
                }
                Some(_) => {}
            }

            let attribute_start = cursor;
            while bytes
                .get(cursor)
                .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'=' | b'>'))
            {
                cursor += 1;
            }
            if cursor == attribute_start {
                // An `=` with no name: skip it rather than stalling.
                cursor += 1;
                continue;
            }
            let attribute = html[attribute_start..cursor].to_ascii_lowercase();

            let mut lookahead = cursor;
            while bytes.get(lookahead).is_some_and(u8::is_ascii_whitespace) {
                lookahead += 1;
            }
            if bytes.get(lookahead) != Some(&b'=') {
                attributes.push((attribute, String::new()));
                continue;
            }
            lookahead += 1;
            while bytes.get(lookahead).is_some_and(u8::is_ascii_whitespace) {
                lookahead += 1;
            }

            let value = match bytes.get(lookahead) {
                Some(quote @ (b'"' | b'\'')) => {
                    let quote = *quote;
                    let start = lookahead + 1;
                    let mut end = start;
                    while bytes.get(end).is_some_and(|byte| *byte != quote) {
                        end += 1;
                    }
                    cursor = (end + 1).min(html.len());
                    html[start.min(html.len())..end].to_owned()
                }
                Some(_) => {
                    let start = lookahead;
                    let mut end = start;
                    while bytes
                        .get(end)
                        .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'>')
                    {
                        end += 1;
                    }
                    cursor = end;
                    html[start..end].to_owned()
                }
                None => {
                    cursor = lookahead;
                    String::new()
                }
            };
            attributes.push((attribute, value));
        }

        tags.push(StartTag { name, attributes });
        index = cursor;
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEDIA: [(&str, &str); 2] = [("img", "src"), ("iframe", "src")];

    #[test]
    fn references_come_back_in_document_order_across_tag_kinds() {
        let html = "<img src=\"first.svg\"><iframe src=\"second.html\"></iframe>\
             <img src=\"third.svg\">";
        assert_eq!(
            html_references(html, &MEDIA),
            [
                ("img".to_owned(), "first.svg".to_owned()),
                ("iframe".to_owned(), "second.html".to_owned()),
                ("img".to_owned(), "third.svg".to_owned()),
            ],
            "a page is read top to bottom, not one tag kind after another"
        );
    }

    #[test]
    fn a_value_may_be_double_quoted_single_quoted_or_unquoted() {
        let html = "<img src=\"double.svg\"><img src='single.svg'><img src=unquoted.svg>\
             <img src=unquoted.svg alt=after><img src='self closed.svg'/>";
        assert_eq!(
            html_references(html, &MEDIA)
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>(),
            [
                "double.svg",
                "single.svg",
                "unquoted.svg",
                "unquoted.svg",
                "self closed.svg",
            ]
        );
        assert_eq!(
            html_references("<img src=trailing.svg/>", &MEDIA),
            [("img".to_owned(), "trailing.svg/".to_owned())],
            "the slash of a self-closing tag belongs to an unquoted value, \
             exactly as a browser reads it"
        );
    }

    #[test]
    fn an_attribute_name_matches_only_at_a_boundary() {
        let html = "<img data-src=\"decoy.svg\" src=\"real.svg\">\
             <img srcset=\"decoy2.svg\" src=\"real2.svg\">\
             <img data-src=\"only-a-decoy.svg\">";
        assert_eq!(
            html_references(html, &MEDIA),
            [
                ("img".to_owned(), "real.svg".to_owned()),
                ("img".to_owned(), "real2.svg".to_owned()),
            ],
            "a preceding look-alike attribute neither matches nor hides the real one"
        );
    }

    #[test]
    fn a_tag_name_matches_only_at_a_boundary_and_ignores_ascii_case() {
        let html = "<IMG SRC=\"upper.svg\"><Iframe Src='mixed.html'></Iframe>\
             <image src=\"not-an-img.svg\"/><imgx src=\"not-an-img-either.svg\">";
        assert_eq!(
            html_references(html, &MEDIA),
            [
                ("img".to_owned(), "upper.svg".to_owned()),
                ("iframe".to_owned(), "mixed.html".to_owned()),
            ]
        );
    }

    #[test]
    fn only_the_requested_tag_attribute_pairs_are_collected() {
        let html = "<script src=\"script.js\"></script><a href=\"page.html\">x</a>\
             <img src=\"chart.svg\" alt=\"a chart\" width=\"160\">";
        assert_eq!(
            html_references(html, &MEDIA),
            [("img".to_owned(), "chart.svg".to_owned())]
        );
        assert_eq!(
            html_references(html, &[("script", "src"), ("a", "href"), ("img", "src")]),
            [
                ("script".to_owned(), "script.js".to_owned()),
                ("a".to_owned(), "page.html".to_owned()),
                ("img".to_owned(), "chart.svg".to_owned()),
            ],
            "each tag is read once, in document order, whatever the pair order"
        );
    }

    #[test]
    fn a_comment_hides_a_tag_shaped_decoy() {
        let html = "<!-- <img src=\"commented.svg\"> --><img src=\"real.svg\">\
             <!-- unterminated <img src=\"never.svg\">";
        assert_eq!(
            html_references(html, &MEDIA),
            [("img".to_owned(), "real.svg".to_owned())]
        );
    }

    #[test]
    fn a_repeated_attribute_keeps_its_first_value_and_a_valueless_one_is_empty() {
        let html = "<img src=\"first.svg\" src=\"second.svg\"><img hidden src=\"after.svg\">\
             <img src>";
        assert_eq!(
            html_references(html, &MEDIA)
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>(),
            ["first.svg", "after.svg", ""]
        );
    }

    #[test]
    fn multi_byte_text_and_an_unterminated_tag_do_not_derail_the_scan() {
        let html = "Prose — em dashes and “quotes” — then <img alt=\"ünïcode\" src=\"real.svg\">\
             and a < in prose, then <img src=\"unterminated.svg";
        assert_eq!(
            html_references(html, &MEDIA)
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>(),
            ["real.svg", "unterminated.svg"]
        );
    }
}
