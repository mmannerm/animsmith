//! Drift gate for rendered Markdown links across the user-facing doc
//! set: every rendered link and image in a gated file must resolve.
//! Relative targets must exist in the repo, `#fragment`s — intra-file
//! and cross-file — must match a GitHub-slugged heading in the target,
//! repo `blob/main`/`tree/main` URLs must point at real paths, and
//! files published to crates.io must use absolute URLs only (see
//! RELEASING.md, "Published README and docs links"). Parsing with
//! `pulldown-cmark` (the parser rustdoc uses) means exactly the links
//! GitHub renders are validated: link-shaped text inside fenced or
//! indented code blocks and inline code spans is ignored, while
//! reference-style, multi-line, and angle-bracket links — invisible to
//! the regex gate this test absorbed from
//! `scripts/check-github-community-files.sh` — are covered. External
//! (non-repo http) links are out of scope.
//!
//! Position is irrelevant here: every rendered link must resolve
//! wherever it sits. Positional completeness claims (every docs page
//! has a Document-column index row) live in `docs_index.rs`.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const REPO_BLOB_URL: &str = "https://github.com/mmannerm/animsmith/blob/main/";
const REPO_TREE_URL: &str = "https://github.com/mmannerm/animsmith/tree/main/";

/// Gated files where relative in-repo links are the norm. The
/// top-level `docs/*.md` pages join this set by directory listing in
/// the gate test, so a new page is gated without editing this list.
const RELATIVE_LINK_FILES: &[&str] = &[
    "CONTRIBUTING.md",
    "DEVELOPMENT.md",
    "SUPPORT.md",
    "SECURITY.md",
    "AGENTS.md",
    "CLAUDE.md",
    ".agent-instructions/shared.md",
    ".github/PULL_REQUEST_TEMPLATE.md",
    "examples/README.md",
];

/// READMEs bundled into published crates render off-repo (crates.io),
/// so relative links would break there: absolute repo URLs only.
const ABSOLUTE_ONLY_FILES: &[&str] = &[
    "README.md",
    "crates/animsmith-core/README.md",
    "crates/animsmith-gltf/README.md",
    "crates/animsmith-fbx/README.md",
    "crates/animsmith-report/README.md",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// GitHub-flavored-markdown parser options, so the rendered-link set
/// matches what github.com renders (tables carry many of the links).
fn gfm_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_TASKLISTS
}

/// Destinations of every link and image GitHub would render. Text that
/// merely looks like a link (code blocks, code spans) never produces a
/// `Tag::Link`/`Tag::Image` event.
fn rendered_link_destinations(markdown: &str) -> Vec<String> {
    Parser::new_ext(markdown, gfm_options())
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. })
            | Event::Start(Tag::Image { dest_url, .. }) => Some(dest_url.into_string()),
            _ => None,
        })
        .collect()
}

/// GitHub's heading-anchor slug: lowercase, drop everything but
/// letters, digits, `_`, and `-`, and turn spaces into hyphens.
fn github_slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// The anchor set GitHub generates for a page's headings, including
/// the `-1`, `-2`, … suffixes it appends to repeated headings.
fn heading_anchors(markdown: &str) -> BTreeSet<String> {
    let mut anchors = BTreeSet::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut heading_text: Option<String> = None;

    for event in Parser::new_ext(markdown, gfm_options()) {
        match event {
            Event::Start(Tag::Heading { .. }) => heading_text = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(text) = heading_text.take() {
                    let slug = github_slug(&text);
                    let repeats = seen.entry(slug.clone()).or_insert(0);
                    anchors.insert(if *repeats == 0 {
                        slug.clone()
                    } else {
                        format!("{slug}-{repeats}")
                    });
                    *repeats += 1;
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(heading) = heading_text.as_mut() {
                    heading.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(heading) = heading_text.as_mut() {
                    heading.push(' ');
                }
            }
            _ => {}
        }
    }
    anchors
}

fn split_fragment(url: &str) -> (&str, Option<&str>) {
    match url.split_once('#') {
        Some((base, fragment)) => (base, Some(fragment)),
        None => (url, None),
    }
}

/// Heading anchors of an existing file, cached by canonical path so a
/// page linked from many gated files is parsed once.
fn cached_anchors<'a>(
    cache: &'a mut BTreeMap<PathBuf, BTreeSet<String>>,
    path: &Path,
) -> &'a BTreeSet<String> {
    let key = path
        .canonicalize()
        .expect("canonicalizes an existing markdown target");
    cache.entry(key).or_insert_with_key(|key| {
        heading_anchors(&std::fs::read_to_string(key).expect("reads markdown target"))
    })
}

/// Validate every rendered link in one gated file, appending one
/// message per broken link. Anchors are validated only against `.md`
/// targets; fragments on other targets (e.g. source-line anchors) are
/// GitHub-UI concerns a checkout cannot resolve.
fn validate_file(
    root: &Path,
    rel: &str,
    absolute_only: bool,
    cache: &mut BTreeMap<PathBuf, BTreeSet<String>>,
    errors: &mut Vec<String>,
) {
    let path = root.join(rel);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => {
            errors.push(format!("{rel}: gated file is missing"));
            return;
        }
    };

    for url in rendered_link_destinations(&content) {
        if url.is_empty() {
            errors.push(format!("{rel}: rendered link has an empty destination"));
            continue;
        }

        if let Some(fragment) = url.strip_prefix('#') {
            if !cached_anchors(cache, &path).contains(fragment) {
                errors.push(format!(
                    "{rel}: intra-file anchor '{url}' matches no heading"
                ));
            }
            continue;
        }

        if url.starts_with("http://") || url.starts_with("https://") {
            let (base, fragment) = split_fragment(&url);
            if let Some(target) = base.strip_prefix(REPO_BLOB_URL) {
                let target_path = root.join(target);
                if !target_path.is_file() {
                    errors.push(format!("{rel}: links to missing repository file {url}"));
                } else if let Some(fragment) = fragment
                    && target.ends_with(".md")
                    && !cached_anchors(cache, &target_path).contains(fragment)
                {
                    errors.push(format!(
                        "{rel}: anchor in {url} matches no heading in {target}"
                    ));
                }
            } else if let Some(target) = base.strip_prefix(REPO_TREE_URL)
                && !root.join(target).is_dir()
            {
                errors.push(format!(
                    "{rel}: links to missing repository directory {url}"
                ));
            }
            // Other web links are external: out of scope for this gate.
            continue;
        }

        if absolute_only {
            errors.push(format!(
                "{rel}: published file must use absolute links, found {url}"
            ));
            continue;
        }

        let (base, fragment) = split_fragment(&url);
        let target_path = path
            .parent()
            .expect("gated file has a parent directory")
            .join(base);
        if !target_path.exists() {
            errors.push(format!("{rel}: links to missing local target {url}"));
            continue;
        }
        if let Some(fragment) = fragment
            && base.ends_with(".md")
            && target_path.is_file()
            && !cached_anchors(cache, &target_path).contains(fragment)
        {
            errors.push(format!(
                "{rel}: anchor in {url} matches no heading in {base}"
            ));
        }
    }
}

#[test]
fn gated_markdown_links_resolve() {
    let root = repo_root();
    let mut cache = BTreeMap::new();
    let mut errors = Vec::new();

    for rel in ABSOLUTE_ONLY_FILES {
        validate_file(&root, rel, true, &mut cache, &mut errors);
    }
    for rel in RELATIVE_LINK_FILES {
        validate_file(&root, rel, false, &mut cache, &mut errors);
    }

    let mut docs_pages = 0usize;
    for entry in std::fs::read_dir(root.join("docs")).expect("lists docs/") {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8 doc name");
        docs_pages += 1;
        validate_file(
            &root,
            &format!("docs/{name}"),
            false,
            &mut cache,
            &mut errors,
        );
    }
    assert!(docs_pages > 0, "docs/ must carry gated markdown pages");

    assert!(
        errors.is_empty(),
        "broken documentation links:\n{}",
        errors.join("\n")
    );
}

/// The false-failure and false-pass classes of the absorbed regex
/// oracle, pinned against the parser: code-shaped decoys must be
/// invisible, and every rendered link form must be visible.
#[test]
fn oracle_sees_all_rendered_link_forms_and_only_those() {
    let fixture = "Prose with a [rendered](rendered.md) link and an\n\
        ![image](image.png) reference.\n\n\
        A [reference-style link][ref] and an [angle](<angle bracket.md>)\n\
        destination.\n\n\
        [ref]: reference.md#section\n\n\
        A [multi-\nline](multiline.md) link.\n\n\
        ```text\n\
        [fenced](fenced.md)\n\
        ```\n\n\
        Inline `[code span](span.md)` link.\n\n\
        \x20   [indented](indented.md)\n";

    let destinations: BTreeSet<String> = rendered_link_destinations(fixture).into_iter().collect();
    let expected: BTreeSet<String> = [
        "rendered.md",
        "image.png",
        "reference.md#section",
        "angle bracket.md",
        "multiline.md",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        destinations, expected,
        "rendered links only — code-block and code-span decoys must not count"
    );
}

/// The anchor forms the docs actually link, matched against GitHub's
/// slugger: numbered headings, punctuation and code spans, kept
/// underscores, and `-1` deduplication of repeated headings.
#[test]
fn heading_anchors_follow_github_slugging() {
    let fixture = "# 1. A first CLI gate\n\n\
        ## CI comments (`lint --format markdown`)\n\n\
        ## A limb is T-posed, or a bone never moves\n\n\
        ## Workflow\n\n\
        ## Workflow\n\n\
        ## speed_mps & `loop = true`\n";

    let expected: BTreeSet<String> = [
        "1-a-first-cli-gate",
        "ci-comments-lint---format-markdown",
        "a-limb-is-t-posed-or-a-bone-never-moves",
        "workflow",
        "workflow-1",
        "speed_mps--loop--true",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(heading_anchors(fixture), expected);
}

/// End-to-end mutation catalog over a fixture repo: the valid link
/// forms pass, and each defect class fails with its own message.
#[test]
fn fixture_repo_valid_links_pass_and_each_mutation_fails() {
    let dir = tempfile::tempdir().expect("creates fixture repo");
    let root = dir.path();
    std::fs::create_dir_all(root.join("docs")).expect("creates docs/");
    std::fs::write(
        root.join("docs/guide.md"),
        "# Guide\n\n## Real heading\n\nBody with a [self](#real-heading) link.\n",
    )
    .expect("writes guide");
    std::fs::write(
        root.join("good.md"),
        format!(
            "# Good\n\n\
             [anchor](docs/guide.md#real-heading), [dir](docs),\n\
             [blob]({REPO_BLOB_URL}docs/guide.md#real-heading),\n\
             [tree]({REPO_TREE_URL}docs), and\n\
             [external](https://example.com/unchecked#whatever).\n"
        ),
    )
    .expect("writes good");
    std::fs::write(
        root.join("published.md"),
        format!(
            "# Published\n\n\
             [ok](#published) and [ok]({REPO_BLOB_URL}docs/guide.md), but\n\
             [relative](docs/guide.md) breaks the published-file policy.\n"
        ),
    )
    .expect("writes published");
    std::fs::write(
        root.join("bad.md"),
        format!(
            "# Bad\n\n\
             [stale](docs/guide.md#missing-heading)\n\
             [missing][gone]\n\
             [self](#nowhere)\n\
             [blob-stale]({REPO_BLOB_URL}docs/guide.md#missing-heading)\n\
             [blob-missing]({REPO_BLOB_URL}docs/nope.md)\n\
             [tree-missing]({REPO_TREE_URL}nonexistent-dir)\n\
             [empty]()\n\n\
             [gone]: docs/nope.md\n"
        ),
    )
    .expect("writes bad");

    let mut cache = BTreeMap::new();

    let mut errors = Vec::new();
    validate_file(root, "docs/guide.md", false, &mut cache, &mut errors);
    validate_file(root, "good.md", false, &mut cache, &mut errors);
    assert!(errors.is_empty(), "valid fixture must pass: {errors:?}");

    let mut errors = Vec::new();
    validate_file(root, "published.md", true, &mut cache, &mut errors);
    assert_eq!(
        errors,
        vec![
            "published.md: published file must use absolute links, found docs/guide.md".to_owned()
        ],
        "only the relative link violates the absolute-only policy"
    );

    let mut errors = Vec::new();
    validate_file(root, "bad.md", false, &mut cache, &mut errors);
    let expected_fragments = [
        "anchor in docs/guide.md#missing-heading matches no heading in docs/guide.md",
        "links to missing local target docs/nope.md",
        "intra-file anchor '#nowhere' matches no heading",
        &format!("anchor in {REPO_BLOB_URL}docs/guide.md#missing-heading matches no heading"),
        &format!("links to missing repository file {REPO_BLOB_URL}docs/nope.md"),
        &format!("links to missing repository directory {REPO_TREE_URL}nonexistent-dir"),
        "rendered link has an empty destination",
    ];
    for fragment in expected_fragments {
        assert!(
            errors.iter().any(|e| e.contains(fragment)),
            "expected an error containing {fragment:?}; got {errors:#?}"
        );
    }
    assert_eq!(
        errors.len(),
        expected_fragments.len(),
        "each mutation fails exactly once: {errors:#?}"
    );
}
