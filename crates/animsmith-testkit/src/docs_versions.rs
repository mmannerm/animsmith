//! The repository's current-version documentation: the inventory of the
//! documents that claim a version, the reader that locates each claim in
//! their bytes, and the writer that moves every claim to one version.
//!
//! `Cargo.toml`'s `[workspace.package] version` is the authority. The
//! `release_version_docs` gate reads the located claims and compares them
//! to it; the `stage_release_docs` example writes them from it. Both walk
//! the same spans — [`claims`] is the only reader — so validation is
//! exactly "staging would report no change", and a version-shaped string
//! anywhere else on an inventoried page (a historical note, a changelog
//! line, a roadmap record) is invisible to both.
//!
//! The inventory is intentionally explicit: a document enters it because
//! it makes a current-version claim, not because it contains a number.
//! `CHANGELOG.md`, the completed bootstrap in `RELEASING.md`, and roadmap
//! records are therefore outside it.
//!
//! Release preparation has two legitimate states: documentation can be
//! staged for the next patch or minor while `main` still carries the last
//! released workspace version, and the generated release-plz PR then bumps
//! the workspace manifest to that documented version. [`validate`] accepts
//! both in [`ReleaseMode::Staging`], and requires exact manifest equality
//! in [`ReleaseMode::ReleasePr`].

use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::path::Path;
use std::process::Command;

/// The command that rewrites every claim in the inventory from the
/// workspace manifest. Named by the drift gate's failure message so the
/// fix is where the failure is.
pub const STAGE_COMMAND: &str = "cargo run -p animsmith --example stage_release_docs";

/// One inventoried document and the current-version claims it makes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Document {
    /// The repository-relative path.
    pub path: &'static str,
    /// The packages whose `package = "X.Y"` snippet line this document
    /// carries, exactly once each.
    pub packages: &'static [&'static str],
    /// The number of `"tool"` objects this document quotes.
    pub tool_examples: usize,
}

/// Every document that states the current release, with what it states.
///
/// The order is what a disagreement is reported against: the release the
/// documentation describes is the one its first `"tool"` example states,
/// and every other claim is held to it.
pub const INVENTORY: &[Document] = &[
    Document {
        path: "README.md",
        packages: &[
            "animsmith-core",
            "animsmith-gltf",
            "animsmith-fbx",
            "animsmith-engine",
            "animsmith-report",
        ],
        tool_examples: 0,
    },
    Document {
        path: "crates/animsmith-core/README.md",
        packages: &["animsmith-core", "animsmith-gltf"],
        tool_examples: 0,
    },
    Document {
        path: "crates/animsmith-gltf/README.md",
        packages: &["animsmith-core", "animsmith-gltf"],
        tool_examples: 0,
    },
    Document {
        path: "crates/animsmith-fbx/README.md",
        packages: &["animsmith-core", "animsmith-fbx"],
        tool_examples: 0,
    },
    Document {
        path: "crates/animsmith-engine/README.md",
        packages: &["animsmith-core", "animsmith-engine"],
        tool_examples: 0,
    },
    Document {
        path: "crates/animsmith-report/README.md",
        packages: &["animsmith-core", "animsmith-report"],
        tool_examples: 0,
    },
    Document {
        path: "docs/embedding.md",
        packages: &[
            "animsmith-core",
            "animsmith-gltf",
            "animsmith-fbx",
            "animsmith-engine",
            "animsmith-report",
        ],
        tool_examples: 0,
    },
    Document {
        path: "docs/output.md",
        packages: &[],
        tool_examples: 4,
    },
    Document {
        path: "docs/mixamo-tutorial.md",
        packages: &[],
        tool_examples: 1,
    },
    Document {
        path: "examples/README.md",
        packages: &[],
        tool_examples: 1,
    },
];

/// The contents of every inventoried document, keyed by repository path.
pub type Snapshot = BTreeMap<&'static str, String>;

/// A canonical `X.Y.Z` release version.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Version {
    /// The major component.
    pub major: u64,
    /// The minor component.
    pub minor: u64,
    /// The patch component.
    pub patch: u64,
}

impl Version {
    /// Read a canonical `X.Y.Z` version. A leading `v`, a leading zero, a
    /// pre-release suffix, or surrounding whitespace is rejected rather
    /// than normalized, so no spelling of a version can compare equal to
    /// the manifest's without being written the same way.
    pub fn parse(value: &str) -> Result<Self, String> {
        let parts: Vec<_> = value.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("expected X.Y.Z, found {value:?}"));
        }
        let parse = |part: &str| {
            if part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
            {
                return Err(format!("expected canonical X.Y.Z, found {value:?}"));
            }
            part.parse::<u64>()
                .map_err(|_| format!("expected canonical X.Y.Z, found {value:?}"))
        };
        Ok(Self {
            major: parse(parts[0])?,
            minor: parse(parts[1])?,
            patch: parse(parts[2])?,
        })
    }

    /// The `X.Y` requirement a Cargo dependency snippet states.
    pub fn dependency_line(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    /// The next minor release after this one.
    pub fn next_minor(self) -> Self {
        Self {
            major: self.major,
            minor: self.minor + 1,
            patch: 0,
        }
    }

    /// The next patch release after this one.
    pub fn next_patch(self) -> Self {
        Self {
            patch: self.patch + 1,
            ..self
        }
    }

    /// The releases documentation may describe while the workspace
    /// manifest still reads this one: the released version itself, and
    /// the next patch or next minor staged before the release workflow is
    /// dispatched.
    ///
    /// One list serves the acceptance rule and the refusal message, so the
    /// two cannot come to disagree about the window.
    pub fn release_window(self) -> [Self; 3] {
        [self, self.next_patch(), self.next_minor()]
    }

    /// Whether documentation describing this version is legitimate while
    /// the workspace manifest reads `workspace`.
    pub fn is_current_or_next_release_from(self, workspace: Self) -> bool {
        workspace.release_window().contains(&self)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// What one located claim states, and therefore how it is spelled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimKind {
    /// The `X.Y` requirement in a `package = "X.Y"` snippet line.
    Dependency(&'static str),
    /// The `X.Y.Z` string in a quoted `"tool"` object's own `version`.
    Tool,
}

impl ClaimKind {
    /// How this claim spells `version`.
    pub fn render(self, version: Version) -> String {
        match self {
            Self::Dependency(_) => version.dependency_line(),
            Self::Tool => version.to_string(),
        }
    }

    /// The version `text` states in this claim's spelling.
    pub fn parse(self, text: &str) -> Result<Version, String> {
        match self {
            // A requirement names a release line, not a release: `X.Y`
            // admits every patch on it, and `X.Y.0` is the line's name.
            Self::Dependency(_) => Version::parse(&format!("{text}.0"))
                .map_err(|_| format!("expected an X.Y requirement, found {text:?}")),
            Self::Tool => Version::parse(text),
        }
    }

    /// How this claim reads in a message.
    pub fn label(self) -> String {
        match self {
            Self::Dependency(package) => format!("`{package}` dependency requirement"),
            Self::Tool => "`tool.version` example".to_owned(),
        }
    }
}

/// One current-version claim, located in the bytes of its document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claim {
    /// The document the claim is in.
    pub path: &'static str,
    /// What the claim states.
    pub kind: ClaimKind,
    /// The 1-based line the claim sits on.
    pub line: usize,
    /// The byte range of the version text itself — never the syntax
    /// around it, and never anything else on the page.
    pub span: Range<usize>,
    /// The version text as written.
    pub text: String,
}

impl Claim {
    /// The version this claim states.
    pub fn version(&self) -> Result<Version, String> {
        self.kind.parse(&self.text).map_err(|error| {
            format!(
                "{}:{}: {}: {error}",
                self.path,
                self.line,
                self.kind.label()
            )
        })
    }
}

/// One rewritten claim, as the staging tool reports it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    /// The repository-relative document.
    pub path: &'static str,
    /// The 1-based line the claim sits on.
    pub line: usize,
    /// What the claim states.
    pub kind: ClaimKind,
    /// The version text that was there.
    pub from: String,
    /// The version text now there.
    pub to: String,
}

impl fmt::Display for Change {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            path,
            line,
            kind,
            from,
            to,
        } = self;
        write!(formatter, "{path}:{line}: {} {from} -> {to}", kind.label())
    }
}

/// Whether a checkout is an ordinary one staging a release, or the
/// generated release PR that must already agree with its manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseMode {
    /// An ordinary branch: documentation may describe the manifest
    /// version, its next patch, or its next minor.
    Staging,
    /// A generated `release-plz-*` branch: every claim must equal the
    /// bumped workspace manifest.
    ReleasePr,
}

/// The environment variable that states the mode outright.
const RELEASE_PR_VARIABLE: &str = "ANIMSMITH_RELEASE_PR";

/// The pull-request head branch GitHub Actions exports.
const HEAD_REF_VARIABLE: &str = "GITHUB_HEAD_REF";

/// The prefix release-plz gives the branch it generates.
const RELEASE_BRANCH_PREFIX: &str = "release-plz-";

/// The version `root`'s workspace manifest releases.
pub fn workspace_version(root: &Path) -> Result<Version, String> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("reads Cargo.toml: {error}"))?;
    let manifest: toml::Value =
        toml::from_str(&manifest).map_err(|error| format!("parses Cargo.toml: {error}"))?;
    let value = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "Cargo.toml must declare workspace.package.version".to_owned())?;
    Version::parse(value).map_err(|error| format!("Cargo.toml workspace version: {error}"))
}

/// Every inventoried document under `root`, read once.
pub fn documentation_snapshot(root: &Path) -> Result<Snapshot, String> {
    INVENTORY
        .iter()
        .map(|document| {
            std::fs::read_to_string(root.join(document.path))
                .map(|content| (document.path, content))
                .map_err(|error| format!("reads {}: {error}", document.path))
        })
        .collect()
}

/// The 1-based line `offset` falls on.
fn line_of(content: &str, offset: usize) -> usize {
    content[..offset].matches('\n').count() + 1
}

/// Locate the one `package = "X.Y"` snippet line each of `document`'s
/// packages contributes to `content`.
///
/// The claim is the requirement text between the quotes: a line that only
/// mentions the package, or quotes the snippet inside a sentence, is not
/// a snippet line and is left alone.
fn dependency_claims(document: &Document, content: &str, errors: &mut Vec<String>) -> Vec<Claim> {
    let mut claims = Vec::new();
    for &package in document.packages {
        let prefix = format!("{package} = \"");
        let mut located = Vec::new();
        let mut offset = 0;
        for line in content.split_inclusive('\n') {
            let indent = line.len() - line.trim_start().len();
            if let Some(rest) = line.trim().strip_prefix(&prefix)
                && let Some(version) = rest.strip_suffix('"')
            {
                let start = offset + indent + prefix.len();
                located.push(Claim {
                    path: document.path,
                    kind: ClaimKind::Dependency(package),
                    line: line_of(content, start),
                    span: start..start + version.len(),
                    text: version.to_owned(),
                });
            }
            offset += line.len();
        }
        if located.len() != 1 {
            errors.push(format!(
                "{}: expected exactly one current `{package} = \"X.Y\"` snippet, found {}",
                document.path,
                located.len()
            ));
            continue;
        }
        claims.extend(located);
    }
    claims
}

/// Locate the `version` string of every quoted `"tool"` object in
/// `content`.
///
/// Each object is parsed as JSON before its version is located, so a
/// snippet the gate reads is a snippet a reader can paste, and a claim is
/// never extracted from text that only looks like an example.
fn tool_claims(document: &Document, content: &str, errors: &mut Vec<String>) -> Vec<Claim> {
    let path = document.path;
    let objects = match json_objects_after_key(content, "\"tool\"") {
        Ok(objects) => objects,
        Err(error) => {
            errors.push(format!("{path}: {error}"));
            return Vec::new();
        }
    };
    if objects.len() != document.tool_examples {
        errors.push(format!(
            "{path}: expected {} current `tool.version` example(s), found {}",
            document.tool_examples,
            objects.len()
        ));
    }

    let mut claims = Vec::new();
    for object in objects {
        let text = &content[object.clone()];
        let parsed: JsonValue = match serde_json::from_str(text) {
            Ok(parsed) => parsed,
            Err(error) => {
                errors.push(format!("{path}: parses current `tool` example: {error}"));
                continue;
            }
        };
        if parsed.get("name").and_then(JsonValue::as_str) != Some("animsmith") {
            errors.push(format!(
                "{path}: current `tool` example must name animsmith"
            ));
            continue;
        }
        let Some(version) = member_string(text, "version") else {
            errors.push(format!(
                "{path}: current `tool` example must carry a string version"
            ));
            continue;
        };
        let span = object.start + version.start..object.start + version.end;
        claims.push(Claim {
            path,
            kind: ClaimKind::Tool,
            line: line_of(content, span.start),
            text: content[span.clone()].to_owned(),
            span,
        });
    }
    claims
}

/// Every current-version claim the inventory makes in `docs`, in
/// inventory order and, within a document, in document order.
///
/// A document whose claims cannot be located is an error rather than a
/// partial answer: the inventory says what it states, so a duplicated,
/// missing, or unreadable claim is drift in itself.
pub fn claims(docs: &Snapshot) -> Result<Vec<Claim>, Vec<String>> {
    let mut errors = Vec::new();
    let mut claims = Vec::new();
    for document in INVENTORY {
        let content = docs
            .get(document.path)
            .expect("a snapshot holds every inventoried document");
        let mut located = dependency_claims(document, content, &mut errors);
        located.extend(tool_claims(document, content, &mut errors));
        located.sort_by_key(|claim| claim.span.start);
        claims.extend(located);
    }
    if errors.is_empty() {
        Ok(claims)
    } else {
        Err(errors)
    }
}

/// The byte range of the contents of the string literal whose opening
/// quote is at `start`, and the offset just past its closing quote.
fn json_string(text: &str, start: usize) -> Option<(Range<usize>, usize)> {
    let bytes = text.as_bytes();
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            // A JSON escape is always followed by one ASCII byte, so the
            // cursor stays on a character boundary.
            b'\\' => cursor += 2,
            b'"' => return Some((start + 1..cursor, cursor + 1)),
            _ => cursor += 1,
        }
    }
    None
}

/// The byte ranges of the JSON objects introduced by `key` in `content`.
fn json_objects_after_key(content: &str, key: &str) -> Result<Vec<Range<usize>>, String> {
    let bytes = content.as_bytes();
    let mut objects = Vec::new();
    let mut search_from = 0;
    while let Some(relative) = content[search_from..].find(key) {
        let key_start = search_from + relative;
        let mut cursor = key_start + key.len();
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b':') {
            search_from = key_start + key.len();
            continue;
        }
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'{') {
            return Err(format!("{key} must introduce a JSON object"));
        }

        let start = cursor;
        let unclosed = || format!("{key} JSON object is not closed");
        let mut depth = 0usize;
        let mut end = None;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'"' => cursor = json_string(content, cursor).ok_or_else(unclosed)?.1,
                b'{' => {
                    depth += 1;
                    cursor += 1;
                }
                b'}' => {
                    depth -= 1;
                    cursor += 1;
                    if depth == 0 {
                        end = Some(cursor);
                        break;
                    }
                }
                _ => cursor += 1,
            }
        }
        let end = end.ok_or_else(unclosed)?;
        objects.push(start..end);
        search_from = end;
    }
    Ok(objects)
}

/// The byte range of `object`'s own `key` string value, relative to
/// `object`.
///
/// Only the object's own members are considered: a `version` inside a
/// nested object belongs to that object, and a string *value* that reads
/// `version` is not a key. `object` must already have parsed as JSON.
fn member_string(object: &str, key: &str) -> Option<Range<usize>> {
    let bytes = object.as_bytes();
    let mut cursor = 0;
    let mut depth = 0usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' | b'[' => {
                depth += 1;
                cursor += 1;
            }
            b'}' | b']' => {
                depth = depth.checked_sub(1)?;
                cursor += 1;
            }
            b'"' => {
                let (name, after) = json_string(object, cursor)?;
                cursor = after;
                if depth != 1 || object.get(name) != Some(key) {
                    continue;
                }
                let mut value = after;
                while bytes.get(value).is_some_and(u8::is_ascii_whitespace) {
                    value += 1;
                }
                // A member's name is followed by `:`; anything else means
                // this string was a value that happens to read like the
                // key.
                if bytes.get(value) != Some(&b':') {
                    continue;
                }
                value += 1;
                while bytes.get(value).is_some_and(u8::is_ascii_whitespace) {
                    value += 1;
                }
                if bytes.get(value) != Some(&b'"') {
                    return None;
                }
                return json_string(object, value).map(|(range, _)| range);
            }
            _ => cursor += 1,
        }
    }
    None
}

/// The release the documentation describes: the one its first `"tool"`
/// example states. Dependency requirements name that release's line.
fn documented_release(claims: &[Claim]) -> Result<Version, String> {
    claims
        .iter()
        .find(|claim| claim.kind == ClaimKind::Tool)
        .ok_or_else(|| "the inventory states no current `tool.version` example".to_owned())?
        .version()
}

/// Every way `docs` fails to describe one release line consistent with
/// the `workspace` manifest.
///
/// Validation is exactly "staging would report no change": every located
/// claim must already be spelled the way the writer would spell the
/// release the documentation describes, and that release must be one the
/// `mode` allows.
pub fn validate(workspace: Version, docs: &Snapshot, mode: ReleaseMode) -> Vec<String> {
    let claims = match claims(docs) {
        Ok(claims) => claims,
        Err(errors) => return errors,
    };
    let documented = match documented_release(&claims) {
        Ok(documented) => documented,
        Err(error) => return vec![error],
    };

    let mut errors: Vec<_> = claims
        .iter()
        .filter_map(|claim| {
            let expected = claim.kind.render(documented);
            (claim.text != expected).then(|| {
                format!(
                    "{}:{}: {} states {}, expected {expected}",
                    claim.path,
                    claim.line,
                    claim.kind.label(),
                    claim.text
                )
            })
        })
        .collect();

    match mode {
        ReleaseMode::ReleasePr if documented != workspace => errors.push(format!(
            "release-plz PR docs describe {documented}, but Cargo.toml releases {workspace}"
        )),
        ReleaseMode::Staging if !documented.is_current_or_next_release_from(workspace) => {
            errors.push(format!(
                "current docs describe {documented}, but Cargo.toml is {workspace}; docs may describe only the current version, next patch, or next minor"
            ));
        }
        _ => {}
    }
    errors
}

/// The branch `root` has checked out, if git can say.
pub fn current_branch(root: &Path) -> Option<String> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|branch| !branch.is_empty())
}

/// The mode a checkout carrying these signals is in.
///
/// The caller passes the environment and the branch — `|name|
/// std::env::var(name).ok()` and [`current_branch`] for a real checkout —
/// so which variable feeds which signal is decided here, where a test can
/// see it, rather than in an untestable wrapper.
///
/// An explicit `ANIMSMITH_RELEASE_PR` decides outright, so a local
/// diagnostic run can ask for either mode. Otherwise the generated
/// release branch is recognised by name, whether CI exports it as the
/// pull-request head or the checkout simply has it.
pub fn release_mode(
    variable: impl Fn(&str) -> Option<String>,
    branch: Option<&str>,
) -> ReleaseMode {
    if let Some(value) = variable(RELEASE_PR_VARIABLE) {
        return if matches!(value.as_str(), "1" | "true") {
            ReleaseMode::ReleasePr
        } else {
            ReleaseMode::Staging
        };
    }
    let is_release_branch = |name: &str| name.starts_with(RELEASE_BRANCH_PREFIX);
    if variable(HEAD_REF_VARIABLE).is_some_and(|head| is_release_branch(&head))
        || branch.is_some_and(is_release_branch)
    {
        ReleaseMode::ReleasePr
    } else {
        ReleaseMode::Staging
    }
}

/// `docs` with every located claim rewritten to `target`, and the changes
/// that took.
///
/// Only the located spans move: the syntax around a claim, and every
/// other version-shaped string on the page, is copied through unchanged.
/// A document whose claims cannot be located is an error rather than a
/// partial rewrite, so nothing is written when anything is unreadable.
pub fn stage(docs: &Snapshot, target: Version) -> Result<(Snapshot, Vec<Change>), Vec<String>> {
    let claims = claims(docs)?;
    let mut staged = docs.clone();
    let mut changes = Vec::new();
    for document in INVENTORY {
        let content = staged
            .get_mut(document.path)
            .expect("a snapshot holds every inventoried document");
        // Later spans first: replacing one never moves an earlier one.
        for claim in claims
            .iter()
            .filter(|claim| claim.path == document.path)
            .rev()
        {
            let replacement = claim.kind.render(target);
            if claim.text == replacement {
                continue;
            }
            changes.push(Change {
                path: claim.path,
                line: claim.line,
                kind: claim.kind,
                from: claim.text.clone(),
                to: replacement.clone(),
            });
            content.replace_range(claim.span.clone(), &replacement);
        }
    }
    changes.sort_by(|left, right| (left.path, left.line).cmp(&(right.path, right.line)));
    Ok((staged, changes))
}

/// Rewrite every current-version claim under `root` to `target`, and
/// report what moved.
///
/// `target` must be in the release window of the version `root`'s
/// manifest releases — the same window [`validate`] accepts before the
/// release workflow is dispatched. Only documents whose bytes change are
/// written, so a second run writes nothing.
pub fn stage_release_docs(root: &Path, target: Version) -> Result<Vec<Change>, String> {
    let workspace = workspace_version(root)?;
    if !target.is_current_or_next_release_from(workspace) {
        let window = workspace
            .release_window()
            .map(|version| version.to_string())
            .join(", ");
        return Err(format!(
            "{target} is not a release this checkout may document; Cargo.toml releases {workspace}, so the window is {window}"
        ));
    }

    let docs = documentation_snapshot(root)?;
    let (staged, changes) = stage(&docs, target).map_err(|errors| {
        format!(
            "current-version documentation cannot be read:\n- {}",
            errors.join("\n- ")
        )
    })?;

    let mut written: Vec<_> = changes.iter().map(|change| change.path).collect();
    written.dedup();
    for path in written {
        std::fs::write(root.join(path), &staged[path])
            .map_err(|error| format!("writes {path}: {error}"))?;
    }
    Ok(changes)
}

/// The version a `stage_release_docs` invocation targets, or `None` when
/// it named none and the workspace manifest decides.
///
/// Only `--version X.Y.Z` is accepted: the release line to stage is the
/// one decision the tool takes from its caller, and everything else it
/// needs it reads from the repository. A malformed invocation is an
/// error rather than a silently ignored argument.
pub fn requested_version<I>(arguments: I) -> Result<Option<Version>, String>
where
    I: IntoIterator<Item = String>,
{
    let usage = format!("usage: {STAGE_COMMAND} [-- --version X.Y.Z]");
    let mut arguments = arguments.into_iter();
    let Some(flag) = arguments.next() else {
        return Ok(None);
    };
    if flag != "--version" {
        return Err(format!("unexpected argument {flag:?}\n{usage}"));
    }
    let value = arguments
        .next()
        .ok_or_else(|| format!("--version needs a version\n{usage}"))?;
    if let Some(extra) = arguments.next() {
        return Err(format!("unexpected argument {extra:?}\n{usage}"));
    }
    Version::parse(&value)
        .map(Some)
        .map_err(|error| format!("--version: {error}\n{usage}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The objects `key` introduces, as the text they select.
    fn objects(content: &str) -> Result<Vec<&str>, String> {
        json_objects_after_key(content, "\"tool\"")
            .map(|ranges| ranges.into_iter().map(|range| &content[range]).collect())
    }

    #[test]
    fn a_tool_key_that_introduces_no_object_is_an_error() {
        assert_eq!(
            objects("\"tool\": [\"animsmith\"]"),
            Err("\"tool\" must introduce a JSON object".to_owned()),
            "a key the page uses for something else is drift, not a claim"
        );
        assert_eq!(
            objects("\"tool\": { \"name\": \"animsmith\""),
            Err("\"tool\" JSON object is not closed".to_owned()),
            "a truncated example states nothing"
        );
        assert_eq!(
            objects("\"tool\": { \"name\": \"} not the end\" }"),
            Ok(vec!["{ \"name\": \"} not the end\" }"]),
            "a brace inside a string does not close the object"
        );
    }

    #[test]
    fn a_tool_key_without_a_colon_is_prose_rather_than_a_claim() {
        assert_eq!(
            objects("the \"tool\" field, and \"tool\": { \"a\": 1 }"),
            Ok(vec!["{ \"a\": 1 }"]),
            "only the mention that introduces an object is read"
        );
    }

    #[test]
    fn only_an_objects_own_string_member_is_located() {
        let object = "{ \"source\": { \"version\": \"9.9.9\" }, \"version\": \"0.1.2\" }";
        let located = member_string(object, "version").expect("the object's own version");
        assert_eq!(
            &object[located], "0.1.2",
            "a nested version is not this one"
        );

        let value_shaped = "{ \"note\": \"version\", \"version\": \"0.1.2\" }";
        let located = member_string(value_shaped, "version").expect("the object's own version");
        assert_eq!(
            &value_shaped[located], "0.1.2",
            "a string value that reads like the key is not a member name"
        );

        assert_eq!(
            member_string("{ \"version\": 12 }", "version"),
            None,
            "a version that is not a string carries no text to move"
        );
        assert_eq!(
            member_string("{ \"name\": \"animsmith\" }", "version"),
            None,
            "an object without the member states nothing"
        );
    }
}
