//! Drift gate for rendered Markdown links across tracked project docs:
//! every tracked `.md` file outside the explicit agent/research
//! exclusions is gated, and every rendered link and image must resolve.
//! Raw-HTML `<img src>` and `<iframe src>` are gated the same way: the
//! customer pages embed generated charts and reports that Markdown's own
//! image syntax cannot express, and a silently broken `src` is exactly
//! the drift this gate exists to catch.
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

use animsmith_testkit::{docs_html, docs_markdown};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

const REPO_BLOB_URL: &str = "https://github.com/mmannerm/animsmith/blob/main/";
const REPO_TREE_URL: &str = "https://github.com/mmannerm/animsmith/tree/main/";

/// Tracked Markdown outside the maintained project-facing document set.
/// `.claude/` contains agent-runner procedures; `docs/research/` keeps
/// archival source notes rather than product documentation.
const EXCLUDED_MARKDOWN_PREFIXES: &[&str] = &[".claude/", "docs/research/"];

/// The Git index is the sole inclusion authority: staged additions join
/// the gate immediately, while untracked Markdown does not. NUL-delimited
/// output keeps whitespace in tracked paths unambiguous.
fn tracked_markdown_files(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "--cached", "-z", "--", "*.md"])
        .output()
        .map_err(|error| format!("runs git ls-files for Markdown inventory: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files for Markdown inventory failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("tracked Markdown paths must be UTF-8: {error}"))?;
    let mut files: Vec<String> = stdout
        .split_terminator('\0')
        .filter(|rel| {
            !EXCLUDED_MARKDOWN_PREFIXES
                .iter()
                .any(|prefix| rel.starts_with(prefix))
        })
        .map(str::to_owned)
        .collect();
    files.sort();
    Ok(files)
}

/// READMEs bundled into published crates render off-repo (crates.io),
/// so relative links would break there. This classifies files already
/// selected by Git; it is not another inclusion source.
fn is_published_readme(rel: &str) -> bool {
    if rel == "README.md" {
        return true;
    }

    let mut components = Path::new(rel).components();
    matches!(
        (
            components.next(),
            components.next(),
            components.next(),
            components.next()
        ),
        (
            Some(std::path::Component::Normal(first)),
            Some(std::path::Component::Normal(_)),
            Some(std::path::Component::Normal(last)),
            None
        ) if first == "crates" && last == "README.md"
    )
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

/// Lowercased semantic prose tokens from rendered Markdown. Fenced/indented
/// code is excluded so a checklist claim cannot be satisfied by a decoy;
/// inline code remains prose because package and command names use it.
fn rendered_prose_words(markdown: &str) -> String {
    let mut in_code_block = false;
    let mut prose = String::new();
    for event in Parser::new_ext(markdown, gfm_options()) {
        match event {
            Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
            Event::End(TagEnd::CodeBlock) => in_code_block = false,
            Event::Text(text) | Event::Code(text) if !in_code_block => {
                prose.push(' ');
                prose.push_str(&text);
            }
            _ => {}
        }
    }
    prose
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '.' | '-' | '/') {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Destinations of every link GitHub would render. Text that merely
/// looks like a link (code blocks, code spans, images) never produces
/// a `Tag::Link` event. README routing completeness deliberately counts
/// links only, so it asks for this rather than the full reference set.
fn rendered_link_destinations(markdown: &str) -> Vec<String> {
    Parser::new_ext(markdown, gfm_options())
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.into_string()),
            _ => None,
        })
        .collect()
}

/// Raw-HTML references whose destination is a local target to validate.
/// `<img>` carries the generated charts and `<iframe>` the embedded
/// reports; both are raw HTML because Markdown can express neither.
const RAW_HTML_MEDIA: [(&str, &str); 2] = [("img", "src"), ("iframe", "src")];

/// Every destination a page publishes, in document order and in one
/// parse: the rendered links and images, and the `src` of the raw-HTML
/// media it embeds. HTML inside fenced or indented code is
/// `Event::Text`, not `Event::Html`, so a code-shaped decoy stays
/// invisible here exactly as it does for Markdown links.
fn rendered_references(markdown: &str) -> Vec<String> {
    let mut destinations = Vec::new();
    for event in Parser::new_ext(markdown, gfm_options()) {
        match event {
            Event::Start(Tag::Link { dest_url, .. })
            | Event::Start(Tag::Image { dest_url, .. }) => {
                destinations.push(dest_url.into_string());
            }
            Event::Html(html) | Event::InlineHtml(html) => destinations.extend(
                docs_html::html_references(&html, &RAW_HTML_MEDIA)
                    .into_iter()
                    .map(|(_, destination)| destination),
            ),
            _ => {}
        }
    }
    destinations
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

/// README routes that must appear as rendered links. A fragment does
/// not change the route: the previous shell assertions intentionally
/// accepted links such as `docs/cli.md#install`.
fn required_readme_link_routes() -> [String; 9] {
    [
        "https://mmannerm.github.io/animsmith/".to_owned(),
        "https://mmannerm.github.io/animsmith/dev/".to_owned(),
        "https://mmannerm.github.io/animsmith/docs/symptoms/".to_owned(),
        "https://mmannerm.github.io/animsmith/docs/game-ready-clips.html".to_owned(),
        "https://mmannerm.github.io/animsmith/docs/cli.html".to_owned(),
        "https://mmannerm.github.io/animsmith/docs/embedding.html".to_owned(),
        format!("{REPO_BLOB_URL}CONTRIBUTING.md"),
        format!("{REPO_BLOB_URL}DEVELOPMENT.md"),
        "https://docs.rs/animsmith-core".to_owned(),
    ]
}

fn missing_required_readme_link_routes(markdown: &str) -> Vec<String> {
    let rendered_routes: BTreeSet<String> = rendered_link_destinations(markdown)
        .into_iter()
        .map(|url| split_fragment(&url).0.to_owned())
        .collect();

    required_readme_link_routes()
        .into_iter()
        .filter(|route| !rendered_routes.contains(route))
        .collect()
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

/// The Markdown pages the documentation site renders, so an anchor into
/// one survives the trip.
///
/// `scripts/build-docs-site.py` publishes `docs/` and the pages the
/// canonical index's Document column rows from elsewhere (the site's
/// navigation is derived from that column). Every other tracked
/// Markdown file becomes a redirect stub — a meta-refresh to the file on
/// GitHub — and a meta-refresh carries no `#fragment`, so a reader
/// following such a link from the site lands at the top of the file
/// rather than at the section the link names.
struct SitePages {
    docs: PathBuf,
    outside_docs: BTreeSet<PathBuf>,
}

impl SitePages {
    /// Read the canonical index. A checkout without one (the fixture
    /// repositories below) publishes nothing outside `docs/`.
    fn read(root: &Path) -> Self {
        let docs = root.join("docs");
        let outside_docs = std::fs::read_to_string(docs.join("README.md"))
            .map(|index| {
                docs_markdown::document_index_targets(&index)
                    .iter()
                    .filter_map(|destination| docs.join(destination).canonicalize().ok())
                    .collect()
            })
            .unwrap_or_default();
        Self {
            docs: docs.canonicalize().unwrap_or(docs),
            outside_docs,
        }
    }

    /// Whether the site renders this existing target as a page of its
    /// own. A `docs/` page that navigation does not carry fails the site
    /// build outright rather than becoming a stub, so `docs/` counts
    /// whole.
    fn renders(&self, target: &Path) -> bool {
        target.canonicalize().is_ok_and(|target| {
            target.starts_with(&self.docs) || self.outside_docs.contains(&target)
        })
    }
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
    site: &SitePages,
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

    for url in rendered_references(&content) {
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
        if fragment.is_some() && base.ends_with(".md") && !site.renders(&target_path) {
            errors.push(format!(
                "{rel}: the site publishes {base} as a redirect stub, which drops \
                 the anchor in {url}"
            ));
        }
    }
}

/// Validate every tracked Markdown file except the explicit exclusions.
fn validate_repo(root: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    let site = SitePages::read(root);
    let mut cache = BTreeMap::new();
    let mut errors = Vec::new();
    let files = tracked_markdown_files(root)?;

    for rel in &files {
        validate_file(
            root,
            rel,
            is_published_readme(rel),
            &site,
            &mut cache,
            &mut errors,
        );
    }
    Ok((files, errors))
}

#[test]
fn gated_markdown_links_resolve() {
    let root = repo_root();
    let (files, errors) = validate_repo(&root).expect("enumerates tracked Markdown with Git");
    assert_eq!(
        EXCLUDED_MARKDOWN_PREFIXES,
        [".claude/", "docs/research/"],
        "only the two stated non-project-doc trees may be excluded"
    );

    let inventory_output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["ls-files", "-z", "--", "*.md"])
        .output()
        .expect("independently enumerates the repository Markdown inventory");
    assert!(
        inventory_output.status.success(),
        "independent Markdown inventory failed: {}",
        String::from_utf8_lossy(&inventory_output.stderr)
    );
    let inventory = String::from_utf8(inventory_output.stdout).expect("tracked paths are UTF-8");
    let mut expected_files: Vec<String> = inventory
        .split_terminator('\0')
        .filter(|rel| !rel.starts_with(".claude/") && !rel.starts_with("docs/research/"))
        .map(str::to_owned)
        .collect();
    expected_files.sort();
    assert_eq!(
        files, expected_files,
        "the gate must contain every tracked Markdown path except the two stated exclusions"
    );

    for newly_gated in [
        "CHANGELOG.md",
        "DESIGN.md",
        "RELEASING.md",
        "THIRD-PARTY.md",
        "crates/animsmith-core/THIRD-PARTY.md",
        "crates/animsmith-fbx/THIRD-PARTY.md",
        "crates/animsmith-engine/THIRD-PARTY.md",
        "crates/animsmith-gltf/THIRD-PARTY.md",
        "crates/animsmith-report/THIRD-PARTY.md",
        "crates/animsmith/THIRD-PARTY.md",
        "examples/assets/README.md",
        "fuzz/README.md",
    ] {
        assert!(
            files.iter().any(|rel| rel == newly_gated),
            "the widened gate must include {newly_gated}"
        );
    }
    assert!(
        !files.is_empty(),
        "the tracked Markdown gate must not be empty"
    );
    assert!(
        errors.is_empty(),
        "broken documentation links:\n{}",
        errors.join("\n")
    );
}

#[test]
fn release_checklist_distinguishes_library_rustdoc_from_the_bin_only_package() {
    let releasing =
        std::fs::read_to_string(repo_root().join("RELEASING.md")).expect("reads release checklist");
    let prose = rendered_prose_words(&releasing);
    for required in [
        "library crates animsmith-core animsmith-gltf animsmith-fbx animsmith-engine and animsmith-report must also have successful rustdoc builds",
        "published animsmith package is intentionally bin-only docs.rs has no library target to document and its cargo rustdoc --lib failure is an accepted mechanically guarded exemption",
        "verify its docs.rs landing/source/features pages only",
    ] {
        assert!(
            prose.contains(required),
            "RELEASING.md must retain the rendered release policy: {required:?}"
        );
    }
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

    let destinations: BTreeSet<String> = rendered_references(fixture).into_iter().collect();
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

/// The raw-HTML half of the same oracle: every `<img>`/`<iframe>`
/// source the customer pages use is seen, and nothing else is — not a
/// code-shaped decoy, not a same-prefix element, not another element's
/// `src`, and not an attribute that merely ends in `src`.
#[test]
fn oracle_sees_raw_html_media_sources_and_only_those() {
    let fixture = "Prose, then a block image:\n\n\
        <img src=\"chart.svg\" alt=\"a chart\" width=\"160\" align=\"right\">\n\n\
        <iframe src=\"report.html#embed=1&finding=0\" loading=\"lazy\"></iframe>\n\n\
        <img src='single.svg'>\n\n\
        An inline <img src=\"inline.svg\"> source.\n\n\
        <image src=\"not-an-img.svg\"/>\n\n\
        <script src=\"script.js\"></script>\n\n\
        <img data-src=\"lazy.svg\" src=\"real.svg\">\n\n\
        ```html\n\
        <img src=\"fenced.svg\">\n\
        <iframe src=\"fenced.html\"></iframe>\n\
        ```\n\n\
        And an inline `<img src=\"span.svg\">` code span.\n";

    assert_eq!(
        rendered_references(fixture),
        [
            "chart.svg",
            "report.html#embed=1&finding=0",
            "single.svg",
            "inline.svg",
            "real.svg",
        ],
        "every real source is seen and every decoy is invisible"
    );
}

/// README routing requirements belong to the rendered-link gate, not
/// the shell grep gate. Each route must be a real Markdown link; text
/// that only resembles one inside a code span or fenced block cannot
/// satisfy the requirement.
#[test]
fn root_readme_renders_required_routing_links_and_decoys_do_not_count() {
    let readme = std::fs::read_to_string(repo_root().join("README.md")).expect("reads root README");
    let missing = missing_required_readme_link_routes(&readme);
    assert!(
        missing.is_empty(),
        "README.md must render required routing links:\n{}",
        missing.join("\n")
    );

    let required_routes = required_readme_link_routes();
    assert_eq!(
        required_routes,
        [
            "https://mmannerm.github.io/animsmith/".to_owned(),
            "https://mmannerm.github.io/animsmith/dev/".to_owned(),
            "https://mmannerm.github.io/animsmith/docs/symptoms/".to_owned(),
            "https://mmannerm.github.io/animsmith/docs/game-ready-clips.html".to_owned(),
            "https://mmannerm.github.io/animsmith/docs/cli.html".to_owned(),
            "https://mmannerm.github.io/animsmith/docs/embedding.html".to_owned(),
            "https://github.com/mmannerm/animsmith/blob/main/CONTRIBUTING.md".to_owned(),
            "https://github.com/mmannerm/animsmith/blob/main/DEVELOPMENT.md".to_owned(),
            "https://docs.rs/animsmith-core".to_owned(),
        ],
        "the README routing contract must retain all required routes"
    );

    let anchored_routes_fixture = required_routes
        .iter()
        .map(|route| {
            if route.ends_with("docs/cli.md") {
                format!("[rendered]({route}#install)")
            } else {
                format!("[rendered]({route}#section)")
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    assert!(
        missing_required_readme_link_routes(&anchored_routes_fixture).is_empty(),
        "a fragment on any required link must not change its README route"
    );

    for required in required_routes {
        for decoy in [
            format!("`[inline decoy]({required})`"),
            format!("```text\n[fenced decoy]({required})\n```"),
            format!("![image decoy]({required})"),
        ] {
            let mut fixture = required_readme_link_routes()
                .into_iter()
                .filter(|route| route != &required)
                .map(|route| format!("[rendered]({route})"))
                .collect::<Vec<_>>()
                .join("\n\n");
            fixture.push_str("\n\n");
            fixture.push_str(&decoy);

            assert_eq!(
                missing_required_readme_link_routes(&fixture),
                vec![required.clone()],
                "a code-shaped decoy must not satisfy {required}"
            );
        }
    }
}

/// README rendered-link presence belongs exclusively to this parser
/// gate. The shell gate may retain ordering checks over README text,
/// but must not regain literal or regex presence checks for links.
#[test]
fn shell_gate_does_not_own_readme_link_presence() {
    let script =
        std::fs::read_to_string(repo_root().join("scripts/check-github-community-files.sh"))
            .expect("reads community-file gate");
    let readme_assertions: Vec<&str> = script
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#') && line.contains("README.md"))
        .collect();

    assert_eq!(
        readme_assertions,
        [
            "require_order README.md \"cargo install animsmith\" \"CONTRIBUTING.md\"",
            "require_order README.md \"animsmith lint clip.glb\" \"CONTRIBUTING.md\"",
        ],
        "the shell gate may retain only its two non-Markdown README ordering contracts"
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
    // The canonical index is the site's navigation authority: a rowed
    // page outside docs/ is a chapter, an unrowed one is a redirect stub.
    std::fs::write(
        root.join("docs/README.md"),
        "# Documentation\n\n| Document | Use it to… |\n|---|---|\n\
         | [Cookbook](../rowed.md) | Do the work. |\n",
    )
    .expect("writes canonical index");
    std::fs::write(root.join("rowed.md"), "# Cookbook\n\n## A section\n")
        .expect("writes rowed page");
    std::fs::write(root.join("unrowed.md"), "# Notes\n\n## A section\n")
        .expect("writes unrowed page");
    std::fs::write(root.join("src.rs"), "// a non-markdown link target\n").expect("writes src.rs");
    std::fs::write(root.join("report.html"), "<p>a generated report</p>\n").expect("writes report");
    std::fs::write(
        root.join("good.md"),
        format!(
            "# Good\n\n\
             [anchor](docs/guide.md#real-heading), [dir](docs),\n\
             [dedupe](docs/guide.md#workflow-1),\n\
             [chapter](rowed.md#a-section), [stub](unrowed.md),\n\
             [blob]({REPO_BLOB_URL}docs/guide.md#real-heading),\n\
             [tree]({REPO_TREE_URL}docs), and\n\
             [external](https://example.com/unchecked#whatever).\n\n\
             Fragments on non-markdown targets are GitHub-UI anchors,\n\
             skipped: [line](src.rs#L1) and [blob-line]({REPO_BLOB_URL}src.rs#L1).\n\n\
             Raw-HTML media resolves like a rendered link, and an\n\
             iframe deep link keeps its report fragment:\n\n\
             <img src=\"src.rs\" alt=\"a local reference\" width=\"10\">\n\n\
             <iframe src=\"report.html#embed=1&finding=0\"></iframe>\n\n\
             Code-shaped decoys around broken targets stay invisible:\n\n\
             ```text\n\
             [fenced decoy](never-a-file.md)\n\
             <img src=\"never-an-image.png\">\n\
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
             [relative](docs/guide.md) breaks the published-file policy.\n\n\
             <img src=\"src.rs\" alt=\"so does a relative raw-HTML source\">\n"
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
             [stub-anchor](unrowed.md#a-section)\n\
             [empty]()\n\n\
             Rendered position is irrelevant — table, list, and\n\
             blockquote links are validated too:\n\n\
             | Broken in a table |\n\
             | --- |\n\
             | [table](missing-table.md) |\n\n\
             - [list](missing-list.md)\n\n\
             > [quote](missing-quote.md)\n\n\
             <img src=\"missing-raw.svg\" alt=\"broken chart\">\n\n\
             <iframe src=\"missing-frame.html#embed=1\"></iframe>\n\n\
             [gone]: docs/nope.md\n"
        ),
    )
    .expect("writes bad");
    std::fs::write(
        root.join("bad2.md"),
        "# Bad Two\n\nAlso links [stale](docs/guide.md#missing-heading).\n",
    )
    .expect("writes bad2");

    let site = SitePages::read(root);
    let mut cache = BTreeMap::new();

    let mut errors = Vec::new();
    validate_file(root, "docs/guide.md", false, &site, &mut cache, &mut errors);
    validate_file(root, "good.md", false, &site, &mut cache, &mut errors);
    assert!(errors.is_empty(), "valid fixture must pass: {errors:?}");

    let mut errors = Vec::new();
    validate_file(root, "published.md", true, &site, &mut cache, &mut errors);
    assert_eq!(
        errors,
        vec![
            "published.md: published file must use absolute links, found docs/guide.md".to_owned(),
            "published.md: published file must use absolute links, found src.rs".to_owned(),
        ],
        "the absolute-only policy covers rendered links and raw-HTML sources alike"
    );

    let mut errors = Vec::new();
    validate_file(root, "bad.md", false, &site, &mut cache, &mut errors);
    validate_file(root, "bad2.md", false, &site, &mut cache, &mut errors);
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
        "links to missing local target missing-raw.svg",
        "links to missing local target missing-frame.html",
        "anchor in #nowhere matches no heading in bad.md",
        &format!("anchor in {REPO_BLOB_URL}docs/guide.md#missing-heading matches no heading"),
        &format!("links to missing repository file {REPO_BLOB_URL}docs/nope.md"),
        &format!("links to missing repository directory {REPO_TREE_URL}nonexistent-dir"),
        "the site publishes unrowed.md as a redirect stub, which drops the anchor \
         in unrowed.md#a-section",
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

/// The Git index is the only inclusion source: arbitrary tracked files
/// join anywhere, untracked/non-Markdown files do not, exclusions match
/// exact directory prefixes, and published README policy is an overlay.
#[test]
fn fixture_repo_gates_all_tracked_markdown_minus_exact_exclusions() {
    let dir = tempfile::tempdir().expect("creates fixture repo");
    let root = dir.path();
    for directory in [
        ".agent-instructions",
        ".claude",
        "crates/example",
        "crates/example/docs",
        "crates/second",
        "docs/research",
        "docs/research_extra",
        "nested/deep",
    ] {
        std::fs::create_dir_all(root.join(directory)).expect("creates fixture directory");
    }

    let git = |args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .expect("runs fixture git");
        assert!(
            output.status.success(),
            "fixture git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "--quiet"]);

    std::fs::write(
        root.join("nested/deep/page.md"),
        "# Deep\n\n[broken](missing-deep.md)\n",
    )
    .expect("writes arbitrary nested Markdown");
    std::fs::write(
        root.join(".agent-instructions/shared.md"),
        "# Shared\n\n[broken](missing-agent.md)\n",
    )
    .expect("writes non-excluded dot directory Markdown");
    std::fs::write(
        root.join(".claude/audit.md"),
        "# Internal\n\n[excluded](missing-internal.md)\n",
    )
    .expect("writes excluded agent Markdown");
    std::fs::write(
        root.join(".claude-notes.md"),
        "# Adjacent\n\n[broken](missing-adjacent.md)\n",
    )
    .expect("writes adjacent dotfile Markdown");
    std::fs::write(
        root.join("docs/research/raw.md"),
        "# Research\n\n[excluded](missing-research.md)\n",
    )
    .expect("writes excluded research Markdown");
    std::fs::write(
        root.join("docs/research-notes.md"),
        "# Adjacent\n\n[broken](missing-research-notes.md)\n",
    )
    .expect("writes adjacent research Markdown");
    std::fs::write(
        root.join("docs/research_extra/page.md"),
        "# Adjacent directory\n\n[broken](missing-research-extra.md)\n",
    )
    .expect("writes adjacent research directory Markdown");
    std::fs::write(root.join("target.txt"), "target\n").expect("writes valid link target");
    std::fs::write(root.join("README.md"), "# Root\n\n[relative](target.txt)\n")
        .expect("writes root README");
    std::fs::write(
        root.join("crates/example/README.md"),
        "# Crate\n\n[relative](../../target.txt)\n",
    )
    .expect("writes crate README");
    std::fs::write(
        root.join("crates/second/README.md"),
        "# Second crate\n\n[relative](../../target.txt)\n",
    )
    .expect("writes second crate README");
    std::fs::write(
        root.join("crates/example/docs/README.md"),
        "# Nested crate docs\n\n[relative](../../../target.txt)\n",
    )
    .expect("writes non-published nested crate README");
    std::fs::write(
        root.join("nested/README.md"),
        "# Ordinary README\n\n[relative](../target.txt)\n",
    )
    .expect("writes ordinary README");
    std::fs::write(
        root.join("untracked.md"),
        "# Untracked\n\n[ignored](missing-untracked.md)\n",
    )
    .expect("writes untracked Markdown");
    std::fs::write(
        root.join("notes.txt"),
        "[not Markdown inventory](missing-notes.md)\n",
    )
    .expect("writes tracked non-Markdown");

    git(&[
        "add",
        "--",
        ".agent-instructions/shared.md",
        ".claude/audit.md",
        ".claude-notes.md",
        "README.md",
        "crates/example/README.md",
        "crates/example/docs/README.md",
        "crates/second/README.md",
        "docs/research/raw.md",
        "docs/research-notes.md",
        "docs/research_extra/page.md",
        "nested/README.md",
        "nested/deep/page.md",
        "notes.txt",
        "target.txt",
    ]);

    let (files, errors) = validate_repo(root).expect("enumerates fixture Git index");
    assert_eq!(
        files,
        [
            ".agent-instructions/shared.md",
            ".claude-notes.md",
            "README.md",
            "crates/example/README.md",
            "crates/example/docs/README.md",
            "crates/second/README.md",
            "docs/research-notes.md",
            "docs/research_extra/page.md",
            "nested/README.md",
            "nested/deep/page.md",
        ],
        "only tracked Markdown outside the exact exclusions is gated"
    );
    assert_eq!(
        errors,
        [
            ".agent-instructions/shared.md: links to missing local target missing-agent.md",
            ".claude-notes.md: links to missing local target missing-adjacent.md",
            "README.md: published file must use absolute links, found target.txt",
            "crates/example/README.md: published file must use absolute links, found ../../target.txt",
            "crates/second/README.md: published file must use absolute links, found ../../target.txt",
            "docs/research-notes.md: links to missing local target missing-research-notes.md",
            "docs/research_extra/page.md: links to missing local target missing-research-extra.md",
            "nested/deep/page.md: links to missing local target missing-deep.md",
        ],
        "each newly gated path and both published overlays must have an exact diagnostic"
    );

    std::fs::create_dir_all(root.join("renamed")).expect("creates renamed fixture directory");
    git(&["mv", "nested/deep/page.md", "renamed/page.md"]);
    let (renamed_files, renamed_errors) =
        validate_repo(root).expect("enumerates staged Markdown rename");
    assert!(
        !renamed_files.iter().any(|rel| rel == "nested/deep/page.md")
            && renamed_files.iter().any(|rel| rel == "renamed/page.md"),
        "a staged rename must replace the old gated path with the new path"
    );
    assert!(
        renamed_errors
            .iter()
            .any(|error| error == "renamed/page.md: links to missing local target missing-deep.md")
            && !renamed_errors
                .iter()
                .any(|error| error.starts_with("nested/deep/page.md:")),
        "diagnostics must follow the staged Markdown rename: {renamed_errors:#?}"
    );

    git(&["rm", "--quiet", "--force", "renamed/page.md"]);
    let (deleted_files, deleted_errors) =
        validate_repo(root).expect("enumerates staged Markdown deletion");
    assert!(
        !deleted_files
            .iter()
            .any(|rel| rel == "nested/deep/page.md" || rel == "renamed/page.md"),
        "a staged deletion must leave no stale gated path"
    );
    assert!(
        !deleted_errors
            .iter()
            .any(|error| error.starts_with("nested/deep/page.md:")
                || error.starts_with("renamed/page.md:")),
        "a staged deletion must leave no stale missing-file diagnostic: {deleted_errors:#?}"
    );
}
