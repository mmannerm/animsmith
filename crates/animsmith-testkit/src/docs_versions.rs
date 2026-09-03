//! The repository's current-version documentation: the inventory of the
//! documents that claim a version, the reader that locates each claim in
//! their bytes, and the writer that moves every claim to one version.
//!
//! `Cargo.toml`'s `[workspace.package] version` is the authority. The
//! `release_version_docs` gate reads the located claims and compares them
//! to it; the `stage_release_docs` example writes them from it. Both walk
//! the same spans, so the tool and the gate cannot disagree about what a
//! document claims, and a version-shaped string anywhere else on an
//! inventoried page — a historical note, a changelog line, a roadmap
//! record — is invisible to both.
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
//! both states, and requires exact manifest equality when its caller asks
//! for release-PR strictness.

use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The command that rewrites every claim in the inventory from the
/// workspace manifest. Named by the drift gate's failure message so the
/// fix is where the failure is.
pub const STAGE_COMMAND: &str = "cargo run -p animsmith --example stage_release_docs";

/// Documents whose Cargo snippets state a current dependency requirement,
/// with the packages each one requires. Each pair contributes exactly one
/// `package = "X.Y"` line to its document.
pub const DEPENDENCY_SNIPPETS: &[(&str, &[&str])] = &[
    (
        "README.md",
        &[
            "animsmith-core",
            "animsmith-gltf",
            "animsmith-fbx",
            "animsmith-engine",
            "animsmith-report",
        ],
    ),
    (
        "crates/animsmith-core/README.md",
        &["animsmith-core", "animsmith-gltf"],
    ),
    (
        "crates/animsmith-gltf/README.md",
        &["animsmith-core", "animsmith-gltf"],
    ),
    (
        "crates/animsmith-fbx/README.md",
        &["animsmith-core", "animsmith-fbx"],
    ),
    (
        "crates/animsmith-engine/README.md",
        &["animsmith-core", "animsmith-engine"],
    ),
    (
        "crates/animsmith-report/README.md",
        &["animsmith-core", "animsmith-report"],
    ),
    (
        "docs/embedding.md",
        &[
            "animsmith-core",
            "animsmith-gltf",
            "animsmith-fbx",
            "animsmith-engine",
            "animsmith-report",
        ],
    ),
];

/// Documents that quote machine-readable output, with the number of
/// `"tool"` objects each one shows. The count is part of the inventory:
/// an example that stops being quoted, or a new one nobody staged, is a
/// change to what the documentation claims.
pub const TOOL_VERSION_SNIPPETS: &[(&str, usize)] = &[
    ("docs/output.md", 4),
    ("docs/mixamo-tutorial.md", 1),
    ("examples/README.md", 1),
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

    /// Whether documentation describing this version is legitimate while
    /// the workspace manifest still reads `workspace`: the released
    /// version itself, or exactly the next patch or next minor staged
    /// before the release workflow is dispatched.
    pub fn is_current_or_next_release_from(self, workspace: Self) -> bool {
        self == workspace
            || (self.major == workspace.major
                && self.minor == workspace.minor
                && self.patch == workspace.patch.saturating_add(1))
            || (self.major == workspace.major
                && self.minor == workspace.minor.saturating_add(1)
                && self.patch == 0)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// What one located claim states, and therefore how it is spelled.
#[derive(Clone, Copy, Debug)]
enum ClaimKind {
    /// The `X.Y` requirement in a `package = "X.Y"` snippet line.
    Dependency(&'static str),
    /// The `X.Y.Z` string in a quoted `"tool"` object's `version`.
    Tool,
}

impl ClaimKind {
    /// How this claim spells `version`.
    fn render(self, version: Version) -> String {
        match self {
            Self::Dependency(_) => version.dependency_line(),
            Self::Tool => version.to_string(),
        }
    }

    /// How this claim reads in a message.
    fn label(self) -> String {
        match self {
            Self::Dependency(package) => format!("`{package}` dependency requirement"),
            Self::Tool => "`tool.version` example".to_owned(),
        }
    }
}

/// One current-version claim, located in the bytes of its document.
#[derive(Debug)]
struct Claim {
    kind: ClaimKind,
    /// The 1-based line the claim sits on.
    line: usize,
    /// The byte range of the version text itself — never the syntax
    /// around it, and never anything else on the page.
    range: Range<usize>,
}

/// One rewritten claim, as the staging tool reports it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    /// The repository-relative document.
    pub path: &'static str,
    /// The 1-based line the claim sits on.
    pub line: usize,
    /// What the claim states, for a human reading the report.
    pub claim: String,
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
            claim,
            from,
            to,
        } = self;
        write!(formatter, "{path}:{line}: {claim} {from} -> {to}")
    }
}

/// The repository root of the checkout this crate was compiled from.
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

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
    inventory_paths()
        .into_iter()
        .map(|path| {
            std::fs::read_to_string(root.join(path))
                .map(|content| (path, content))
                .map_err(|error| format!("reads {path}: {error}"))
        })
        .collect()
}

/// Every document in the inventory, once each, in inventory order.
///
/// The order is what a disagreement is reported against: the first claim
/// read is the release line the rest are held to, so it is the inventory's
/// own order rather than an incidental sort.
fn inventory_paths() -> Vec<&'static str> {
    let mut paths = Vec::new();
    for path in DEPENDENCY_SNIPPETS
        .iter()
        .map(|(path, _)| *path)
        .chain(TOOL_VERSION_SNIPPETS.iter().map(|(path, _)| *path))
    {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

/// The 1-based line `offset` falls on.
fn line_of(content: &str, offset: usize) -> usize {
    content[..offset].matches('\n').count() + 1
}

/// Locate the one `package = "X.Y"` snippet line each inventoried package
/// contributes to `content`.
///
/// The claim is the requirement text between the quotes: a line that only
/// mentions the package, or quotes the snippet inside a sentence, is not
/// a snippet line and is left alone.
fn dependency_claims(
    path: &str,
    packages: &[&'static str],
    content: &str,
    errors: &mut Vec<String>,
) -> Vec<Claim> {
    let mut claims = Vec::new();
    for &package in packages {
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
                    kind: ClaimKind::Dependency(package),
                    line: line_of(content, start),
                    range: start..start + version.len(),
                });
            }
            offset += line.len();
        }
        if located.len() != 1 {
            errors.push(format!(
                "{path}: expected exactly one current `{package} = \"X.Y\"` snippet, found {}",
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
fn tool_claims(
    path: &str,
    expected_count: usize,
    content: &str,
    errors: &mut Vec<String>,
) -> Vec<Claim> {
    let objects = match json_objects_after_key(content, "\"tool\"") {
        Ok(objects) => objects,
        Err(error) => {
            errors.push(format!("{path}: {error}"));
            return Vec::new();
        }
    };
    if objects.len() != expected_count {
        errors.push(format!(
            "{path}: expected {expected_count} current `tool.version` example(s), found {}",
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
        let Some(version) = string_value_range(text, "version") else {
            errors.push(format!(
                "{path}: current `tool` example must carry a string version"
            ));
            continue;
        };
        let range = object.start + version.start..object.start + version.end;
        claims.push(Claim {
            kind: ClaimKind::Tool,
            line: line_of(content, range.start),
            range,
        });
    }
    claims
}

/// Every current-version claim one inventoried document makes, in
/// document order.
fn document_claims(path: &str, content: &str, errors: &mut Vec<String>) -> Vec<Claim> {
    let mut claims = Vec::new();
    for &(inventoried, packages) in DEPENDENCY_SNIPPETS {
        if inventoried == path {
            claims.extend(dependency_claims(path, packages, content, errors));
        }
    }
    for &(inventoried, expected_count) in TOOL_VERSION_SNIPPETS {
        if inventoried == path {
            claims.extend(tool_claims(path, expected_count, content, errors));
        }
    }
    claims.sort_by_key(|claim| claim.range.start);
    claims
}

/// Every claim in the inventory, by document. A document missing from
/// `docs` is an error rather than an empty result: the inventory says it
/// makes a claim.
fn snapshot_claims(docs: &Snapshot, errors: &mut Vec<String>) -> Vec<(&'static str, Vec<Claim>)> {
    let mut claims = Vec::new();
    for path in inventory_paths() {
        let Some(content) = docs.get(path) else {
            errors.push(format!("{path}: current-version document is missing"));
            continue;
        };
        claims.push((path, document_claims(path, content, errors)));
    }
    claims
}

/// The byte ranges of the JSON objects introduced by `key` in `content`.
fn json_objects_after_key(content: &str, key: &str) -> Result<Vec<Range<usize>>, String> {
    let mut objects = Vec::new();
    let mut search_from = 0;
    while let Some(relative) = content[search_from..].find(key) {
        let key_start = search_from + relative;
        let mut cursor = key_start + key.len();
        let bytes = content.as_bytes();
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
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        let mut end = None;
        for (offset, byte) in bytes[start..].iter().copied().enumerate() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_string = false;
                }
                continue;
            }
            match byte {
                b'"' => in_string = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + offset + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end.ok_or_else(|| format!("{key} JSON object is not closed"))?;
        objects.push(start..end);
        search_from = end;
    }
    Ok(objects)
}

/// The byte range of the contents of the string literal whose opening
/// quote is at `start`, and the offset just past its closing quote.
fn json_string(text: &str, start: usize) -> Option<(Range<usize>, usize)> {
    let bytes = text.as_bytes();
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor += 2,
            b'"' => return Some((start + 1..cursor, cursor + 1)),
            _ => cursor += 1,
        }
    }
    None
}

/// The byte range of `object`'s own `key` string value, relative to
/// `object`.
///
/// Only the object's own members are considered: a `version` inside a
/// nested object belongs to that object, not to this one. `object` must
/// already have parsed as JSON.
fn string_value_range(object: &str, key: &str) -> Option<Range<usize>> {
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
                if depth != 1 || &object[name] != key {
                    continue;
                }
                let mut value = after;
                while bytes.get(value).is_some_and(u8::is_ascii_whitespace) {
                    value += 1;
                }
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

/// The version every located claim in `docs` states, in document order.
fn read_claims(
    docs: &Snapshot,
    errors: &mut Vec<String>,
) -> Vec<(&'static str, ClaimKind, Version)> {
    let mut versions = Vec::new();
    for (path, located) in snapshot_claims(docs, errors) {
        let content = &docs[path];
        for claim in located {
            let text = &content[claim.range.clone()];
            let parsed = match claim.kind {
                ClaimKind::Dependency(package) => Version::parse(&format!("{text}.0")).map_err(|_| {
                    format!(
                        "{path}:{}: `{package}` dependency must use an X.Y requirement, found {text:?}",
                        claim.line
                    )
                }),
                ClaimKind::Tool => Version::parse(text).map_err(|error| {
                    format!("{path}: current `tool.version` {text:?} is invalid: {error}")
                }),
            };
            match parsed {
                Ok(version) => versions.push((path, claim.kind, version)),
                Err(error) => errors.push(error),
            }
        }
    }
    versions
}

/// Every way `docs` fails to describe one release line consistent with
/// the `workspace` manifest.
///
/// `require_manifest_match` is the release-PR rule: on a generated
/// `release-plz-*` branch the documented version must equal the bumped
/// manifest exactly. Off it, pre-dispatch staging of the next patch or
/// minor is also accepted.
pub fn validate(workspace: Version, docs: &Snapshot, require_manifest_match: bool) -> Vec<String> {
    let mut errors = Vec::new();
    let mut dependencies = Vec::new();
    let mut tools = Vec::new();
    for (path, kind, version) in read_claims(docs, &mut errors) {
        match kind {
            ClaimKind::Dependency(package) => dependencies.push((path, package, version)),
            ClaimKind::Tool => tools.push((path, version)),
        }
    }

    let dependency_version = dependencies.first().map(|(_, _, version)| *version);
    if let Some(expected) = dependency_version {
        for &(path, package, found) in &dependencies {
            if found != expected {
                errors.push(format!(
                    "{path}: `{package}` uses dependency line {}, expected {}",
                    found.dependency_line(),
                    expected.dependency_line()
                ));
            }
        }
    }

    let tool_version = tools.first().map(|(_, version)| *version);
    if let Some(expected) = tool_version {
        for &(path, found) in &tools {
            if found != expected {
                errors.push(format!(
                    "{path}: `tool.version` is {found}, expected {expected}"
                ));
            }
        }
        if require_manifest_match && expected != workspace {
            errors.push(format!(
                "release-plz PR docs describe {expected}, but Cargo.toml releases {workspace}"
            ));
        } else if !require_manifest_match && !expected.is_current_or_next_release_from(workspace) {
            errors.push(format!(
                "current docs describe {expected}, but Cargo.toml is {workspace}; docs may describe only the current version, next patch, or next minor"
            ));
        }
    }

    if let (Some(dependency), Some(tool)) = (dependency_version, tool_version)
        && (dependency.major, dependency.minor) != (tool.major, tool.minor)
    {
        errors.push(format!(
            "dependency snippets use {}, but current `tool.version` examples use {}",
            dependency.dependency_line(),
            tool.dependency_line()
        ));
    }

    errors
}

/// Whether this checkout is a generated release PR, in which every
/// current-version claim must equal the bumped manifest.
pub fn is_release_plz_pr(root: &Path) -> bool {
    let branch = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
    strict_release_mode(
        std::env::var("ANIMSMITH_RELEASE_PR").ok().as_deref(),
        std::env::var("GITHUB_HEAD_REF").ok().as_deref(),
        branch.as_deref(),
    )
}

/// The release-PR decision itself, as a function of the signals a
/// checkout carries: an explicit `ANIMSMITH_RELEASE_PR` override, the
/// pull-request head branch CI exports, and the checked-out branch.
pub fn strict_release_mode(
    explicit: Option<&str>,
    github_head_ref: Option<&str>,
    branch: Option<&str>,
) -> bool {
    if let Some(value) = explicit {
        return matches!(value, "1" | "true");
    }
    github_head_ref.is_some_and(|head| head.starts_with("release-plz-"))
        || branch.is_some_and(|branch| branch.starts_with("release-plz-"))
}

/// `docs` with every located claim rewritten to `target`, and the changes
/// that took.
///
/// Only the located spans move: the syntax around a claim, and every
/// other version-shaped string on the page, is copied through unchanged.
/// A document whose claims cannot be located is an error rather than a
/// partial rewrite.
pub fn stage(docs: &Snapshot, target: Version) -> Result<(Snapshot, Vec<Change>), Vec<String>> {
    let mut errors = Vec::new();
    let claims = snapshot_claims(docs, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut staged = docs.clone();
    let mut changes = Vec::new();
    for (path, located) in claims {
        let content = staged
            .get_mut(path)
            .expect("the snapshot holds every located document");
        // Later spans first: replacing one never moves an earlier one.
        for claim in located.iter().rev() {
            let replacement = claim.kind.render(target);
            let current = &content[claim.range.clone()];
            if current == replacement {
                continue;
            }
            changes.push(Change {
                path,
                line: claim.line,
                claim: claim.kind.label(),
                from: current.to_owned(),
                to: replacement.clone(),
            });
            content.replace_range(claim.range.clone(), &replacement);
        }
    }
    changes.sort_by(|left, right| (left.path, left.line).cmp(&(right.path, right.line)));
    Ok((staged, changes))
}

/// Rewrite every current-version claim under `root` to `target`, and
/// report what moved.
///
/// `target` must be the version `root`'s manifest releases, its next
/// patch, or its next minor — the same window [`validate`] accepts before
/// the release workflow is dispatched. Only documents whose bytes change
/// are written, so a second run writes nothing.
pub fn stage_release_docs(root: &Path, target: Version) -> Result<Vec<Change>, String> {
    let workspace = workspace_version(root)?;
    if !target.is_current_or_next_release_from(workspace) {
        return Err(format!(
            "{target} is neither the workspace version {workspace}, its next patch {}, nor its next minor {}",
            workspace.next_patch(),
            workspace.next_minor()
        ));
    }

    let docs = documentation_snapshot(root)?;
    let (staged, changes) = stage(&docs, target).map_err(|errors| {
        format!(
            "current-version documentation cannot be read:\n- {}",
            errors.join("\n- ")
        )
    })?;

    let written: BTreeSet<_> = changes.iter().map(|change| change.path).collect();
    for path in written {
        std::fs::write(root.join(path), &staged[path])
            .map_err(|error| format!("writes {path}: {error}"))?;
    }
    Ok(changes)
}
