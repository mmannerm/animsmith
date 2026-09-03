//! Release-aware drift gate for current package-version documentation.
//!
//! Release preparation has two legitimate states: documentation can be staged
//! for the next patch or minor while `main` still carries the last released
//! workspace version, and the generated release-plz PR then bumps the
//! workspace manifest to that documented version. This gate accepts both
//! states, but requires exact manifest equality on a `release-plz-*` branch.
//!
//! The inventory, the reader that locates each current-version claim, and the
//! writer that moves them all live in `animsmith-testkit`'s
//! [`docs_versions`](animsmith_testkit::docs_versions) module, so this gate
//! and the `stage_release_docs` example read the same spans. The inventory is
//! intentionally explicit. Historical references in `CHANGELOG.md`, the
//! completed bootstrap in `RELEASING.md`, and roadmap records are not
//! current-version claims and therefore never enter the scan.

use animsmith_testkit::docs_versions::{
    self as versions, DEPENDENCY_SNIPPETS, STAGE_COMMAND, Snapshot, TOOL_VERSION_SNIPPETS, Version,
};
use std::collections::BTreeSet;
use std::path::Path;

/// Replace `from` with `to` in every document of a snapshot copy.
fn replace_all(docs: &mut Snapshot, from: &str, to: &str) {
    for content in docs.values_mut() {
        *content = content.replace(from, to);
    }
}

/// Replace the `occurrence`-th `from` in one document.
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

/// The one dependency line and the one `tool.version` a valid snapshot
/// describes, read back through the writer: staging to a version the
/// documents do not carry reports what each claim moved from.
fn documented_versions(docs: &Snapshot) -> (Version, Version) {
    let probe = Version {
        major: u64::MAX,
        minor: 0,
        patch: 0,
    };
    let (_, changes) = versions::stage(docs, probe).expect("fixture claims are located");
    let mut dependency = None;
    let mut tool = None;
    for change in changes {
        if change.claim.contains("dependency") {
            dependency
                .get_or_insert_with(|| Version::parse(&format!("{}.0", change.from)).expect("X.Y"));
        } else {
            tool.get_or_insert_with(|| Version::parse(&change.from).expect("X.Y.Z"));
        }
    }
    (
        dependency.expect("the inventory states a dependency requirement"),
        tool.expect("the inventory states a tool version"),
    )
}

/// A repository copy holding only what the inventory names, plus the
/// manifest the writer reads its version from.
struct Fixture {
    _directory: tempfile::TempDir,
    root: std::path::PathBuf,
}

impl Fixture {
    /// Copy the inventoried documents and a manifest declaring `version`.
    fn new(version: Version) -> Self {
        let directory = tempfile::tempdir().expect("temporary repository");
        let root = directory.path().to_path_buf();
        std::fs::write(
            root.join("Cargo.toml"),
            format!("[workspace.package]\nversion = \"{version}\"\n"),
        )
        .expect("writes the fixture manifest");
        let repository = versions::repo_root();
        for (path, content) in
            versions::documentation_snapshot(&repository).expect("reads the inventory")
        {
            let destination = root.join(path);
            std::fs::create_dir_all(destination.parent().expect("document directory"))
                .expect("creates the document directory");
            std::fs::write(destination, content).expect("copies the document");
        }
        Self {
            _directory: directory,
            root,
        }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn snapshot(&self) -> Snapshot {
        versions::documentation_snapshot(&self.root).expect("reads the fixture inventory")
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.root.join(path)).expect("reads a fixture document")
    }

    fn write(&self, path: &str, content: &str) {
        std::fs::write(self.root.join(path), content).expect("writes a fixture document");
    }
}

#[test]
fn current_release_version_docs_are_consistent() {
    let root = versions::repo_root();
    let workspace = versions::workspace_version(&root).expect("reads workspace version");
    let docs =
        versions::documentation_snapshot(&root).expect("reads current-version documentation");
    let errors = versions::validate(workspace, &docs, versions::is_release_plz_pr(&root));
    assert!(
        errors.is_empty(),
        "release-version documentation drift (run `{STAGE_COMMAND}`):\n- {}",
        errors.join("\n- ")
    );
}

#[test]
fn pre_dispatch_successors_pass_then_release_pr_requires_exact_manifest() {
    let root = versions::repo_root();
    let workspace = versions::workspace_version(&root).expect("reads workspace version");
    let original =
        versions::documentation_snapshot(&root).expect("reads current-version documentation");

    for (kind, staged) in [
        ("next patch", workspace.next_patch()),
        ("next minor", workspace.next_minor()),
    ] {
        let (docs, _) = versions::stage(&original, staged).expect("stages the successor");

        assert!(
            versions::validate(workspace, &docs, false).is_empty(),
            "pre-dispatch docs may stage the {kind} before release-plz bumps Cargo.toml"
        );
        assert!(
            versions::validate(workspace, &docs, true)
                .iter()
                .any(|error| error.contains(&format!("Cargo.toml releases {workspace}"))),
            "release PR mode must reject {kind} docs that do not match its manifest"
        );
        assert!(
            versions::validate(staged, &docs, true).is_empty(),
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
    assert!(versions::strict_release_mode(Some("1"), None, None));
    assert!(versions::strict_release_mode(Some("true"), None, None));
    assert!(versions::strict_release_mode(
        None,
        Some("release-plz-2026-08-16"),
        None
    ));
    assert!(versions::strict_release_mode(
        None,
        None,
        Some("release-plz-2026-08-16")
    ));
    assert!(!versions::strict_release_mode(
        None,
        Some("feature/docs"),
        Some("main")
    ));
    assert!(
        !versions::strict_release_mode(Some("false"), Some("release-plz-generated"), None),
        "an explicit false override keeps local diagnostic runs non-strict"
    );
}

#[test]
fn successor_policy_rejects_two_patches_ahead_and_cross_domain_drift() {
    let root = versions::repo_root();
    let workspace = versions::workspace_version(&root).expect("reads workspace version");
    let original =
        versions::documentation_snapshot(&root).expect("reads current-version documentation");
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
        versions::validate(workspace, &too_far, false)
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
        versions::validate(workspace, &crossed, false)
            .iter()
            .any(|error| error.contains("dependency snippets use")),
        "individually allowed successors must still describe one release line"
    );
}

#[test]
fn malformed_current_tool_json_is_rejected() {
    let root = versions::repo_root();
    let workspace = versions::workspace_version(&root).expect("reads workspace version");
    let mut docs =
        versions::documentation_snapshot(&root).expect("reads current-version documentation");
    let content = docs.get_mut("docs/mixamo-tutorial.md").expect("tutorial");
    *content = content.replacen(
        "\"name\": \"animsmith\"",
        "\"name\": \"animsmith\", INVALID",
        1,
    );
    assert!(
        versions::validate(workspace, &docs, false)
            .iter()
            .any(|error| error.contains("parses current `tool` example")),
        "the gate must parse the complete tool object rather than extract version text"
    );
}

#[test]
fn every_stale_dependency_and_tool_version_mutation_fails() {
    let root = versions::repo_root();
    let workspace = versions::workspace_version(&root).expect("reads workspace version");
    let docs =
        versions::documentation_snapshot(&root).expect("reads current-version documentation");
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
                let errors = versions::validate(workspace, &mutated, false);
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
                let errors = versions::validate(workspace, &mutated, false);
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

#[test]
fn a_manifest_bump_restates_every_claim_and_leaves_the_stale_copy_failing() {
    let released =
        versions::workspace_version(&versions::repo_root()).expect("reads workspace version");
    // A minor bump moves both claim spellings; a patch bump would leave the
    // `X.Y` dependency requirements alone.
    let bumped = released.next_minor();
    let fixture = Fixture::new(bumped);
    let stale = fixture.snapshot();

    let changes = versions::stage_release_docs(fixture.root(), bumped).expect("stages the bump");
    let inventoried_claims = DEPENDENCY_SNIPPETS
        .iter()
        .map(|(_, packages)| packages.len())
        .sum::<usize>()
        + TOOL_VERSION_SNIPPETS
            .iter()
            .map(|(_, count)| count)
            .sum::<usize>();
    assert_eq!(
        changes.len(),
        inventoried_claims,
        "a manifest bump restates every inventoried claim: {changes:?}"
    );
    let restated: BTreeSet<_> = changes.iter().map(|change| change.path).collect();
    let inventoried: BTreeSet<_> = stale.keys().copied().collect();
    assert_eq!(
        restated, inventoried,
        "every inventoried document is written"
    );
    for change in &changes {
        let expected = if change.claim.contains("dependency") {
            bumped.dependency_line()
        } else {
            bumped.to_string()
        };
        assert_eq!(change.to, expected, "{change} states the bumped version");
    }

    let staged = fixture.snapshot();
    assert_eq!(
        versions::validate(bumped, &staged, true),
        Vec::<String>::new(),
        "the release PR accepts what the writer wrote"
    );
    assert!(
        versions::validate(bumped, &stale, true)
            .iter()
            .any(|error| error.contains(&format!("Cargo.toml releases {bumped}"))),
        "the release PR rejects the pre-generation copy against the bumped manifest"
    );
    assert!(
        versions::validate(bumped, &stale, false)
            .iter()
            .any(|error| error.contains("current docs describe")),
        "a manifest a minor ahead rejects the stale copy outside release-PR strictness too"
    );

    let again = versions::stage_release_docs(fixture.root(), bumped).expect("stages again");
    assert!(again.is_empty(), "a second run changes nothing: {again:?}");
    assert_eq!(
        fixture.snapshot(),
        staged,
        "a second run leaves every document byte-identical"
    );
}

#[test]
fn generation_moves_only_the_claim_spans_and_not_historical_prose() {
    let released =
        versions::workspace_version(&versions::repo_root()).expect("reads workspace version");
    let bumped = released.next_minor();
    let fixture = Fixture::new(bumped);

    // Prose quoting the version being replaced, in the shapes a
    // whole-document search-and-replace would rewrite: the released
    // dependency requirement, a released tool version in a JSON object that
    // is not a `tool` object, and the release's own name.
    let history = format!(
        "\n## History (fixture)\n\nAnimSmith {released} is the release this page was written \
         for. Its manifests read `animsmith-core = \"{line}\"`, and a report from the {line} \
         line carried `\"generator\": {{ \"name\": \"animsmith\", \"version\": \"{released}\" }}`.\n",
        line = released.dependency_line(),
    );
    for path in ["docs/embedding.md", "docs/output.md"] {
        fixture.write(path, &format!("{}{history}", fixture.read(path)));
    }

    let changes = versions::stage_release_docs(fixture.root(), bumped).expect("stages the bump");
    assert!(!changes.is_empty(), "the fixture had claims to restate");
    for path in ["docs/embedding.md", "docs/output.md"] {
        let content = fixture.read(path);
        assert!(
            content.ends_with(&history),
            "{path}: the historical paragraph survives generation verbatim:\n{}",
            &content[content.len().saturating_sub(history.len() * 2)..]
        );
    }
    assert!(
        fixture.read("docs/embedding.md").contains(&format!(
            "animsmith-core = \"{}\"",
            bumped.dependency_line()
        )),
        "the current dependency claim still moved"
    );
    assert_eq!(
        versions::validate(bumped, &fixture.snapshot(), true),
        Vec::<String>::new(),
        "a historical paragraph is not a current-version claim"
    );
}

#[test]
fn a_nested_version_belongs_to_its_own_object() {
    let released =
        versions::workspace_version(&versions::repo_root()).expect("reads workspace version");
    let bumped = released.next_minor();
    let fixture = Fixture::new(bumped);

    // A `version` one level down is another object's claim. It is written
    // before the tool's own, so a reader that took the first `version` it
    // saw inside the object would read and rewrite this one.
    let decoy = "\"source\": { \"revision\": null, \"version\": \"9.9.9\" }, ";
    let tutorial = fixture.read("docs/mixamo-tutorial.md").replacen(
        "\"name\": \"animsmith\"",
        &format!("{decoy}\"name\": \"animsmith\""),
        1,
    );
    fixture.write("docs/mixamo-tutorial.md", &tutorial);

    versions::stage_release_docs(fixture.root(), bumped).expect("stages the bump");
    let staged = fixture.read("docs/mixamo-tutorial.md");
    assert!(
        staged.contains("\"version\": \"9.9.9\""),
        "the nested object keeps its own version"
    );
    assert!(
        staged.contains(&format!(
            "\"name\": \"animsmith\", \"version\": \"{bumped}\""
        )),
        "the tool object's own version is the claim that moved"
    );
    assert_eq!(
        versions::validate(bumped, &fixture.snapshot(), true),
        Vec::<String>::new(),
        "the tool object still states one release"
    );
}

#[test]
fn staging_refuses_a_version_outside_the_release_window() {
    let released =
        versions::workspace_version(&versions::repo_root()).expect("reads workspace version");
    let fixture = Fixture::new(released);
    let before = fixture.snapshot();

    for refused in [
        Version {
            patch: released.patch + 2,
            ..released
        },
        released.next_minor().next_minor(),
        Version {
            major: released.major + 1,
            minor: 0,
            patch: 0,
        },
    ] {
        let error = versions::stage_release_docs(fixture.root(), refused)
            .expect_err("a version outside the release window is refused");
        assert!(
            error.contains(&refused.to_string()),
            "the refusal names the requested version: {error}"
        );
        assert_eq!(
            fixture.snapshot(),
            before,
            "a refused version writes nothing"
        );
    }

    for accepted in [released, released.next_patch(), released.next_minor()] {
        versions::stage_release_docs(fixture.root(), accepted)
            .expect("the manifest version and its next patch and minor are stageable");
        versions::stage_release_docs(fixture.root(), released).expect("restores the fixture");
    }
    assert_eq!(
        fixture.snapshot(),
        before,
        "staging back to the manifest version restores the documents"
    );
}
