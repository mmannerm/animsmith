//! Release-aware drift gate for current package-version documentation.
//!
//! Release preparation has two legitimate states: documentation can be staged
//! for the next patch or minor while `main` still carries the last released
//! workspace version, and the generated release-plz PR then bumps the
//! workspace manifest to that documented version. This gate accepts both
//! states, but requires exact manifest equality on a `release-plz-*` branch.
//!
//! The inventory is intentionally explicit. Historical references in
//! `CHANGELOG.md`, the completed bootstrap in `RELEASING.md`, and roadmap
//! records are not current-version claims and therefore never enter the scan.

use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEPENDENCY_SNIPPETS: &[(&str, &[&str])] = &[
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

const TOOL_VERSION_SNIPPETS: &[(&str, usize)] = &[
    ("docs/output.md", 4),
    ("docs/mixamo-tutorial.md", 1),
    ("examples/README.md", 1),
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    fn parse(value: &str) -> Result<Self, String> {
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

    fn dependency_line(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    fn next_minor(self) -> Self {
        Self {
            major: self.major,
            minor: self.minor + 1,
            patch: 0,
        }
    }

    fn next_patch(self) -> Self {
        Self {
            patch: self.patch + 1,
            ..self
        }
    }

    fn is_current_or_next_release_from(self, workspace: Self) -> bool {
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

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn workspace_version(root: &Path) -> Result<Version, String> {
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

fn documentation_snapshot(root: &Path) -> Result<BTreeMap<&'static str, String>, String> {
    let paths: BTreeSet<_> = DEPENDENCY_SNIPPETS
        .iter()
        .map(|(path, _)| *path)
        .chain(TOOL_VERSION_SNIPPETS.iter().map(|(path, _)| *path))
        .collect();
    paths
        .into_iter()
        .map(|path| {
            std::fs::read_to_string(root.join(path))
                .map(|content| (path, content))
                .map_err(|error| format!("reads {path}: {error}"))
        })
        .collect()
}

fn dependency_versions(
    docs: &BTreeMap<&str, String>,
    errors: &mut Vec<String>,
) -> Vec<(&'static str, &'static str, Version)> {
    let mut versions = Vec::new();
    for &(path, packages) in DEPENDENCY_SNIPPETS {
        let Some(content) = docs.get(path) else {
            errors.push(format!("{path}: current-version document is missing"));
            continue;
        };
        for &package in packages {
            let prefix = format!("{package} = \"");
            let matches: Vec<_> = content
                .lines()
                .enumerate()
                .filter_map(|(index, line)| {
                    line.trim()
                        .strip_prefix(&prefix)
                        .and_then(|rest| rest.strip_suffix('"'))
                        .map(|version| (index + 1, version))
                })
                .collect();
            if matches.len() != 1 {
                errors.push(format!(
                    "{path}: expected exactly one current `{package} = \"X.Y\"` snippet, found {}",
                    matches.len()
                ));
                continue;
            }
            let (line, version) = matches[0];
            match Version::parse(&format!("{version}.0")) {
                Ok(version) => versions.push((path, package, version)),
                Err(_) => errors.push(format!(
                    "{path}:{line}: `{package}` dependency must use an X.Y requirement, found {version:?}"
                )),
            }
        }
    }
    versions
}

fn json_objects_after_key<'a>(content: &'a str, key: &str) -> Result<Vec<&'a str>, String> {
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
        objects.push(&content[start..end]);
        search_from = end;
    }
    Ok(objects)
}

fn tool_versions(
    docs: &BTreeMap<&str, String>,
    errors: &mut Vec<String>,
) -> Vec<(&'static str, Version)> {
    let mut versions = Vec::new();
    for &(path, expected_count) in TOOL_VERSION_SNIPPETS {
        let Some(content) = docs.get(path) else {
            errors.push(format!("{path}: current-version document is missing"));
            continue;
        };
        let objects = match json_objects_after_key(content, "\"tool\"") {
            Ok(objects) => objects,
            Err(error) => {
                errors.push(format!("{path}: {error}"));
                continue;
            }
        };
        if objects.len() != expected_count {
            errors.push(format!(
                "{path}: expected {expected_count} current `tool.version` example(s), found {}",
                objects.len()
            ));
        }
        for object in objects {
            let parsed: JsonValue = match serde_json::from_str(object) {
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
            let Some(version) = parsed.get("version").and_then(JsonValue::as_str) else {
                errors.push(format!(
                    "{path}: current `tool` example must carry a string version"
                ));
                continue;
            };
            match Version::parse(version) {
                Ok(version) => versions.push((path, version)),
                Err(error) => errors.push(format!(
                    "{path}: current `tool.version` {version:?} is invalid: {error}"
                )),
            }
        }
    }
    versions
}

fn validate_snapshot(
    workspace: Version,
    docs: &BTreeMap<&str, String>,
    require_manifest_match: bool,
) -> Vec<String> {
    let mut errors = Vec::new();
    let dependencies = dependency_versions(docs, &mut errors);
    let tools = tool_versions(docs, &mut errors);

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

fn is_release_plz_pr(root: &Path) -> bool {
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

fn strict_release_mode(
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

fn replace_all(docs: &mut BTreeMap<&str, String>, from: &str, to: &str) {
    for content in docs.values_mut() {
        *content = content.replace(from, to);
    }
}

fn replace_nth(content: &str, from: &str, to: &str, occurrence: usize) -> String {
    let start = content
        .match_indices(from)
        .nth(occurrence)
        .map(|(start, _)| start)
        .expect("mutation occurrence exists");
    let mut mutated = content.to_owned();
    mutated.replace_range(start..start + from.len(), to);
    mutated
}

fn documented_versions(docs: &BTreeMap<&str, String>) -> (Version, Version) {
    let mut errors = Vec::new();
    let dependency = dependency_versions(docs, &mut errors)
        .first()
        .map(|(_, _, version)| *version)
        .expect("fixture carries dependency snippets");
    let tool = tool_versions(docs, &mut errors)
        .first()
        .map(|(_, version)| *version)
        .expect("fixture carries tool.version examples");
    assert!(errors.is_empty(), "fixture inventory is valid: {errors:?}");
    (dependency, tool)
}

#[test]
fn current_release_version_docs_are_consistent() {
    let root = repo_root();
    let workspace = workspace_version(&root).expect("reads workspace version");
    let docs = documentation_snapshot(&root).expect("reads current-version documentation");
    let errors = validate_snapshot(workspace, &docs, is_release_plz_pr(&root));
    assert!(
        errors.is_empty(),
        "release-version documentation drift:\n- {}",
        errors.join("\n- ")
    );
}

#[test]
fn pre_dispatch_successors_pass_then_release_pr_requires_exact_manifest() {
    let root = repo_root();
    let workspace = workspace_version(&root).expect("reads workspace version");
    let original = documentation_snapshot(&root).expect("reads current-version documentation");
    let (documented_dependency, documented_tool) = documented_versions(&original);

    for (kind, staged) in [
        ("next patch", workspace.next_patch()),
        ("next minor", workspace.next_minor()),
    ] {
        let mut docs = original.clone();
        replace_all(
            &mut docs,
            &format!(" = \"{}\"", documented_dependency.dependency_line()),
            &format!(" = \"{}\"", staged.dependency_line()),
        );
        replace_all(
            &mut docs,
            &format!("\"version\": \"{documented_tool}\""),
            &format!("\"version\": \"{staged}\""),
        );

        assert!(
            validate_snapshot(workspace, &docs, false).is_empty(),
            "pre-dispatch docs may stage the {kind} before release-plz bumps Cargo.toml"
        );
        assert!(
            validate_snapshot(workspace, &docs, true)
                .iter()
                .any(|error| error.contains(&format!("Cargo.toml releases {workspace}"))),
            "release PR mode must reject {kind} docs that do not match its manifest"
        );
        assert!(
            validate_snapshot(staged, &docs, true).is_empty(),
            "the generated {kind} release PR passes once its manifest and staged docs agree"
        );
    }
}

#[test]
fn acceptance_inventory_names_every_current_version_document() {
    let dependency_paths: BTreeSet<_> = DEPENDENCY_SNIPPETS.iter().map(|(path, _)| *path).collect();
    assert_eq!(
        dependency_paths,
        [
            "README.md",
            "crates/animsmith-core/README.md",
            "crates/animsmith-fbx/README.md",
            "crates/animsmith-engine/README.md",
            "crates/animsmith-gltf/README.md",
            "crates/animsmith-report/README.md",
            "docs/embedding.md",
        ]
        .into_iter()
        .collect(),
        "the acceptance-criteria dependency document inventory is exact"
    );
    assert_eq!(
        TOOL_VERSION_SNIPPETS,
        &[
            ("docs/output.md", 4),
            ("docs/mixamo-tutorial.md", 1),
            ("examples/README.md", 1),
        ],
        "the acceptance-criteria tool.version document inventory is exact"
    );
}

#[test]
fn version_comparison_rejects_noncanonical_semver_spelling() {
    for version in [
        "00.2.1",
        "0.02.1",
        "0.2.01",
        "+0.2.1",
        "v0.2.1",
        " 0.2.1",
        "0.2.1 ",
        "0.2.1-alpha",
    ] {
        assert!(
            Version::parse(version).is_err(),
            "noncanonical version {version:?} must not compare equal to a manifest version"
        );
    }
}

#[test]
fn every_release_context_signal_selects_strict_mode() {
    assert!(strict_release_mode(Some("1"), None, None));
    assert!(strict_release_mode(Some("true"), None, None));
    assert!(strict_release_mode(
        None,
        Some("release-plz-2026-08-16"),
        None
    ));
    assert!(strict_release_mode(
        None,
        None,
        Some("release-plz-2026-08-16")
    ));
    assert!(!strict_release_mode(
        None,
        Some("feature/docs"),
        Some("main")
    ));
    assert!(
        !strict_release_mode(Some("false"), Some("release-plz-generated"), None),
        "an explicit false override keeps local diagnostic runs non-strict"
    );
}

#[test]
fn successor_policy_rejects_two_patches_ahead_and_cross_domain_drift() {
    let root = repo_root();
    let workspace = workspace_version(&root).expect("reads workspace version");
    let original = documentation_snapshot(&root).expect("reads current-version documentation");
    let (documented_dependency, documented_tool) = documented_versions(&original);

    let two_patches = Version {
        patch: workspace.patch + 2,
        ..workspace
    };
    let mut too_far = original.clone();
    replace_all(
        &mut too_far,
        &format!("\"version\": \"{documented_tool}\""),
        &format!("\"version\": \"{two_patches}\""),
    );
    assert!(
        validate_snapshot(workspace, &too_far, false)
            .iter()
            .any(|error| error.contains("current docs describe")),
        "ordinary main may not stage two patch releases ahead"
    );

    let dependency_next_minor = workspace.next_minor();
    let tool_next_patch = workspace.next_patch();
    let mut crossed = original;
    replace_all(
        &mut crossed,
        &format!(" = \"{}\"", documented_dependency.dependency_line()),
        &format!(" = \"{}\"", dependency_next_minor.dependency_line()),
    );
    replace_all(
        &mut crossed,
        &format!("\"version\": \"{documented_tool}\""),
        &format!("\"version\": \"{tool_next_patch}\""),
    );
    assert!(
        validate_snapshot(workspace, &crossed, false)
            .iter()
            .any(|error| error.contains("dependency snippets use")),
        "individually allowed successors must still describe one release line"
    );
}

#[test]
fn malformed_current_tool_json_is_rejected() {
    let root = repo_root();
    let workspace = workspace_version(&root).expect("reads workspace version");
    let mut docs = documentation_snapshot(&root).expect("reads current-version documentation");
    let content = docs.get_mut("docs/mixamo-tutorial.md").expect("tutorial");
    *content = content.replacen(
        "\"name\": \"animsmith\"",
        "\"name\": \"animsmith\", INVALID",
        1,
    );
    assert!(
        validate_snapshot(workspace, &docs, false)
            .iter()
            .any(|error| error.contains("parses current `tool` example")),
        "the gate must parse the complete tool object rather than extract version text"
    );
}

#[test]
fn every_stale_dependency_and_tool_version_mutation_fails() {
    let root = repo_root();
    let workspace = workspace_version(&root).expect("reads workspace version");
    let docs = documentation_snapshot(&root).expect("reads current-version documentation");
    let (documented_dependency, documented_tool) = documented_versions(&docs);

    for &(path, packages) in DEPENDENCY_SNIPPETS {
        for &package in packages {
            let stale = docs.clone();
            let from = format!(
                "{package} = \"{}\"",
                documented_dependency.dependency_line()
            );
            for stale_version in ["0.0", "999.999"] {
                let mut mutated = stale.clone();
                let content = mutated.get_mut(path).unwrap();
                *content = replace_nth(
                    content,
                    &from,
                    &format!("{package} = \"{stale_version}\""),
                    0,
                );
                let errors = validate_snapshot(workspace, &mutated, false);
                assert!(
                    errors
                        .iter()
                        .any(|error| error.contains(path) && error.contains(package)),
                    "stale {package} dependency {stale_version} in {path} must fail: {errors:?}"
                );
            }
        }
    }

    let from = format!("\"version\": \"{documented_tool}\"");
    for &(path, count) in TOOL_VERSION_SNIPPETS {
        for occurrence in 0..count {
            let stale = docs.clone();
            for stale_version in ["0.0.0", "999.999.999"] {
                let mut mutated = stale.clone();
                let content = mutated.get_mut(path).unwrap();
                *content = replace_nth(
                    content,
                    &from,
                    &format!("\"version\": \"{stale_version}\""),
                    occurrence,
                );
                let errors = validate_snapshot(workspace, &mutated, false);
                assert!(
                    errors
                        .iter()
                        .any(|error| error.contains(path) && error.contains("tool.version")),
                    "stale tool.version {stale_version} occurrence {occurrence} in {path} must fail: {errors:?}"
                );
            }
        }
    }
}
