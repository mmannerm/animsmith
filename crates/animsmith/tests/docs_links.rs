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
/// `validate_repo`, so a new page is gated without editing this list.
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
/// so relative links would break there: absolute repo URLs only. The
/// root README is the crates.io front page for the `animsmith` CLI
/// crate; crate READMEs join by directory listing, so a new crate
/// README is gated without editing any list.
fn absolute_only_files(root: &Path) -> Vec<String> {
    let mut files = vec!["README.md".to_owned()];
    if let Ok(entries) = std::fs::read_dir(root.join("crates")) {
        for entry in entries {
            let entry = entry.expect("directory entry");
            if entry.path().join("README.md").is_file() {
                let name = entry.file_name();
                files.push(format!(
                    "crates/{}/README.md",
                    name.to_str().expect("utf-8 crate dir")
                ));
            }
        }
    }
    files.sort();
    files
}

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
/// Deduplication follows github-slugger: a suffixed candidate that
/// collides with an anchor already taken (e.g. a literal `Workflow 1`
/// heading followed by duplicate `Workflow`s, or vice versa) keeps
/// bumping the base counter until the candidate is free, so `Workflow`,
/// `Workflow`, `Workflow 1` yields `workflow`, `workflow-1`,
/// `workflow-1-1`.
fn heading_anchors(markdown: &str) -> BTreeSet<String> {
    let mut anchors = BTreeSet::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut heading_text: Option<String> = None;

    for event in Parser::new_ext(markdown, gfm_options()) {
        match event {
            Event::Start(Tag::Heading { .. }) => heading_text = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(text) = heading_text.take() {
                    let base = github_slug(&text);
                    let mut candidate = base.clone();
                    while anchors.contains(&candidate) {
                        let count = counts.entry(base.clone()).or_insert(0);
                        *count += 1;
                        candidate = format!("{base}-{count}");
                    }
                    anchors.insert(candidate);
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

/// The one anchor rule shared by the intra-file, `blob/main`, and
/// relative branches: a `#fragment` on an existing `.md` target must
/// name one of its headings. Fragments on non-`.md` targets (e.g.
/// source-line anchors) are GitHub-UI concerns a checkout cannot
/// resolve, so they are skipped.
fn check_md_anchor(
    cache: &mut BTreeMap<PathBuf, BTreeSet<String>>,
    target_path: &Path,
    target_display: &str,
    fragment: Option<&str>,
    rel: &str,
    url: &str,
    errors: &mut Vec<String>,
) {
    if let Some(fragment) = fragment
        && target_display.ends_with(".md")
        && target_path.is_file()
        && !cached_anchors(cache, target_path).contains(fragment)
    {
        errors.push(format!(
            "{rel}: anchor in {url} matches no heading in {target_display}"
        ));
    }
}

/// Validate every rendered link in one gated file, appending one
/// message per broken link.
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
            check_md_anchor(cache, &path, rel, Some(fragment), rel, &url, errors);
            continue;
        }

        if url.starts_with("http://") || url.starts_with("https://") {
            let (base, fragment) = split_fragment(&url);
            if let Some(target) = base.strip_prefix(REPO_BLOB_URL) {
                let target_path = root.join(target);
                if !target_path.is_file() {
                    errors.push(format!("{rel}: links to missing repository file {url}"));
                } else {
                    check_md_anchor(cache, &target_path, target, fragment, rel, &url, errors);
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
        check_md_anchor(cache, &target_path, base, fragment, rel, &url, errors);
    }
}

/// Walk the full gated set under `root`: the published (absolute-only)
/// READMEs, the curated relative-link files, and every top-level
/// `docs/*.md` by directory listing. Returns the docs-page count so
/// callers can assert the enumeration saw a non-empty docs set.
fn validate_repo(root: &Path) -> (usize, Vec<String>) {
    let mut cache = BTreeMap::new();
    let mut errors = Vec::new();

    for rel in absolute_only_files(root) {
        validate_file(root, &rel, true, &mut cache, &mut errors);
    }
    for rel in RELATIVE_LINK_FILES {
        validate_file(root, rel, false, &mut cache, &mut errors);
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
            root,
            &format!("docs/{name}"),
            false,
            &mut cache,
            &mut errors,
        );
    }
    (docs_pages, errors)
}

#[test]
fn gated_markdown_links_resolve() {
    let root = repo_root();
    assert!(
        absolute_only_files(&root).len() > 1,
        "crates/ must carry published-crate READMEs"
    );

    let (docs_pages, errors) = validate_repo(&root);
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
        ## Workflow 1\n\n\
        ## speed_mps & `loop = true`\n";

    let expected: BTreeSet<String> = [
        "1-a-first-cli-gate",
        "ci-comments-lint---format-markdown",
        "a-limb-is-t-posed-or-a-bone-never-moves",
        "workflow",
        "workflow-1",
        // The literal `Workflow 1` heading collides with the suffixed
        // duplicate above it; github-slugger resolves it to -1-1.
        "workflow-1-1",
        "speed_mps--loop--true",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(heading_anchors(fixture), expected);
}

/// End-to-end mutation catalog over a fixture repo: the valid link
/// forms pass — including code-shaped decoys around broken targets,
/// dedupe-suffix anchors, and skipped fragments on non-`.md` targets —
/// and each defect class fails with its own message.
#[test]
fn fixture_repo_valid_links_pass_and_each_mutation_fails() {
    let dir = tempfile::tempdir().expect("creates fixture repo");
    let root = dir.path();
    std::fs::create_dir_all(root.join("docs")).expect("creates docs/");
    std::fs::write(
        root.join("docs/guide.md"),
        "# Guide\n\n## Real heading\n\n## Workflow\n\n## Workflow\n\n\
         Body with a [self](#real-heading) link.\n",
    )
    .expect("writes guide");
    std::fs::write(root.join("src.rs"), "// a non-markdown link target\n").expect("writes src.rs");
    std::fs::write(
        root.join("good.md"),
        format!(
            "# Good\n\n\
             [anchor](docs/guide.md#real-heading), [dir](docs),\n\
             [dedupe](docs/guide.md#workflow-1),\n\
             [blob]({REPO_BLOB_URL}docs/guide.md#real-heading),\n\
             [tree]({REPO_TREE_URL}docs), and\n\
             [external](https://example.com/unchecked#whatever).\n\n\
             Fragments on non-markdown targets are GitHub-UI anchors,\n\
             skipped: [line](src.rs#L1) and [blob-line]({REPO_BLOB_URL}src.rs#L1).\n\n\
             Code-shaped decoys around broken targets stay invisible:\n\n\
             ```text\n\
             [fenced decoy](never-a-file.md)\n\
             ```\n\n\
             and an inline `[span decoy](also-never.md)` link.\n"
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
             [dedupe-stale](docs/guide.md#workflow-2)\n\
             [missing][gone]\n\
             [multi-\nline](missing-multiline.md)\n\
             [angle](<missing angle.md>)\n\
             ![image](missing.png)\n\
             [self](#nowhere)\n\
             [blob-stale]({REPO_BLOB_URL}docs/guide.md#missing-heading)\n\
             [blob-missing]({REPO_BLOB_URL}docs/nope.md)\n\
             [tree-missing]({REPO_TREE_URL}nonexistent-dir)\n\
             [empty]()\n\n\
             Rendered position is irrelevant — table, list, and\n\
             blockquote links are validated too:\n\n\
             | Broken in a table |\n\
             | --- |\n\
             | [table](missing-table.md) |\n\n\
             - [list](missing-list.md)\n\n\
             > [quote](missing-quote.md)\n\n\
             [gone]: docs/nope.md\n"
        ),
    )
    .expect("writes bad");
    std::fs::write(
        root.join("bad2.md"),
        "# Bad Two\n\nAlso links [stale](docs/guide.md#missing-heading).\n",
    )
    .expect("writes bad2");

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
    validate_file(root, "bad2.md", false, &mut cache, &mut errors);
    let expected_fragments = [
        // Every linking source is named: the same stale heading is
        // reported once per file that links it.
        "bad.md: anchor in docs/guide.md#missing-heading matches no heading in docs/guide.md",
        "bad2.md: anchor in docs/guide.md#missing-heading matches no heading in docs/guide.md",
        "anchor in docs/guide.md#workflow-2 matches no heading in docs/guide.md",
        "links to missing local target docs/nope.md",
        "links to missing local target missing-multiline.md",
        "links to missing local target missing angle.md",
        "links to missing local target missing.png",
        "links to missing local target missing-table.md",
        "links to missing local target missing-list.md",
        "links to missing local target missing-quote.md",
        "anchor in #nowhere matches no heading in bad.md",
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

/// The enumeration wiring itself, pinned on a fixture with a fully
/// independent oracle: every expected diagnostic below is a literal
/// string, never derived from the implementation's own lists, so
/// dropping any member from the gated set — a community file from
/// `RELATIVE_LINK_FILES`, the docs directory listing, the crate-README
/// discovery, or the root README — fails a fixed assertion here.
/// Every fixture docs page carries its own named diagnostic (an
/// implementation that merely counts a page without validating it
/// fails), and `docs/extra.md` is an arbitrary, non-mandated name (an
/// implementation hard-coding the well-known docs page names fails).
#[test]
fn fixture_repo_enumeration_gates_mandated_members_and_missing_files() {
    let dir = tempfile::tempdir().expect("creates fixture repo");
    let root = dir.path();
    std::fs::create_dir_all(root.join("docs")).expect("creates docs/");
    std::fs::create_dir_all(root.join("examples")).expect("creates examples/");
    std::fs::create_dir_all(root.join("crates/mycrate")).expect("creates crates/mycrate/");
    std::fs::write(
        root.join("docs/README.md"),
        "# Documentation\n\nThe index links [broken](missing-index.md).\n",
    )
    .expect("writes docs index");
    std::fs::write(
        root.join("docs/game-ready-clips.md"),
        "# Game-ready clips\n\nA symptom links [broken](missing-symptom.md).\n",
    )
    .expect("writes symptom guide");
    std::fs::write(
        root.join("docs/extra.md"),
        "# Extra\n\nA new page links [broken](missing-extra.md).\n",
    )
    .expect("writes extra page");
    std::fs::write(
        root.join("examples/README.md"),
        "# Examples\n\nA cookbook links [broken](missing-example.md).\n",
    )
    .expect("writes examples readme");
    std::fs::write(
        root.join("crates/mycrate/README.md"),
        "# mycrate\n\nA published README links [relative](../../docs/README.md).\n",
    )
    .expect("writes crate readme");

    let (docs_pages, errors) = validate_repo(root);
    assert_eq!(docs_pages, 3, "directory listing must see all docs pages");

    let expected_diagnostics = [
        "docs/README.md: links to missing local target missing-index.md",
        "docs/game-ready-clips.md: links to missing local target missing-symptom.md",
        "docs/extra.md: links to missing local target missing-extra.md",
        "examples/README.md: links to missing local target missing-example.md",
        "crates/mycrate/README.md: published file must use absolute links, \
         found ../../docs/README.md",
    ];
    let expected_missing = [
        "CONTRIBUTING.md",
        "DEVELOPMENT.md",
        "SUPPORT.md",
        "SECURITY.md",
        "AGENTS.md",
        "CLAUDE.md",
        ".agent-instructions/shared.md",
        ".github/PULL_REQUEST_TEMPLATE.md",
        "README.md",
    ];
    for required in expected_diagnostics {
        assert!(
            errors.iter().any(|e| e == required),
            "mandated gated member must be validated: {required:?}; got {errors:#?}"
        );
    }
    for rel in expected_missing {
        assert!(
            errors
                .iter()
                .any(|e| e == &format!("{rel}: gated file is missing")),
            "absent gated file {rel} must be an error; got {errors:#?}"
        );
    }
    assert_eq!(
        errors.len(),
        expected_diagnostics.len() + expected_missing.len(),
        "exactly the member diagnostics plus the missing gated files: {errors:#?}"
    );
}
