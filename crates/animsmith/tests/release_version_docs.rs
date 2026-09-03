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
//! and the `stage_release_docs` example read the same spans: validating is
//! asking whether staging would report a change. The inventory is
//! intentionally explicit, and a tree scan below holds it to the tracked
//! Markdown. Historical references in `CHANGELOG.md`, the completed bootstrap
//! in `RELEASING.md`, and `DESIGN.md`'s roadmap are not current-version
//! claims and therefore never enter it.

use animsmith_testkit::docs_versions::{
    self as versions, Claim, ClaimKind, Document, INVENTORY, ReleaseMode, STAGE_COMMAND, Snapshot,
    Version,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// The repository's own current-version documentation.
fn repository_docs() -> Snapshot {
    versions::documentation_snapshot(&animsmith_testkit::repo_root())
        .expect("reads current-version documentation")
}

/// Every claim the repository's documentation makes.
fn repository_claims() -> Vec<Claim> {
    versions::claims(&repository_docs()).expect("the repository inventory is readable")
}

/// The release the repository's documentation describes, which is the
/// released version or a staged successor of it.
fn documented_release() -> Version {
    repository_claims()
        .iter()
        .find(|claim| claim.kind == ClaimKind::Tool)
        .expect("the inventory quotes a tool example")
        .version()
        .expect("the quoted tool version is canonical")
}

/// The number of claims the inventory makes.
fn inventoried_claims() -> usize {
    INVENTORY
        .iter()
        .map(|document| document.packages.len() + document.tool_examples)
        .sum()
}

/// A repository copy holding only what the inventory names, plus the
/// manifest the writer reads its version from.
struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    /// Copy the inventoried documents beside a manifest declaring
    /// `manifest`.
    ///
    /// The documents state whatever the repository currently stages, so a
    /// test that wants them to agree with the manifest passes
    /// [`documented_release`] here.
    fn new(manifest: Version) -> Self {
        let directory = tempfile::tempdir().expect("temporary repository");
        let root = directory.path().to_path_buf();
        std::fs::write(
            root.join("Cargo.toml"),
            format!("[workspace.package]\nversion = \"{manifest}\"\n"),
        )
        .expect("writes the fixture manifest");
        for (path, content) in repository_docs() {
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
    let root = animsmith_testkit::repo_root();
    let workspace = versions::workspace_version(&root).expect("reads workspace version");
    let mode = versions::release_mode(
        |name| std::env::var(name).ok(),
        versions::current_branch(&root).as_deref(),
    );
    let errors = versions::validate(workspace, &repository_docs(), mode);
    assert!(
        errors.is_empty(),
        "release-version documentation drift (run `{STAGE_COMMAND}`):\n- {}",
        errors.join("\n- ")
    );
}

#[test]
fn pre_dispatch_successors_pass_then_release_pr_requires_exact_manifest() {
    let root = animsmith_testkit::repo_root();
    let workspace = versions::workspace_version(&root).expect("reads workspace version");
    let original = repository_docs();

    for (kind, staged) in [
        ("next patch", workspace.next_patch()),
        ("next minor", workspace.next_minor()),
    ] {
        let (docs, _) = versions::stage(&original, staged).expect("stages the successor");

        assert!(
            versions::validate(workspace, &docs, ReleaseMode::Staging).is_empty(),
            "pre-dispatch docs may stage the {kind} before release-plz bumps Cargo.toml"
        );
        assert!(
            versions::validate(workspace, &docs, ReleaseMode::ReleasePr)
                .iter()
                .any(|error| error.contains(&format!("Cargo.toml releases {workspace}"))),
            "release PR mode must reject {kind} docs that do not match its manifest"
        );
        assert!(
            versions::validate(staged, &docs, ReleaseMode::ReleasePr).is_empty(),
            "the generated {kind} release PR passes once its manifest and staged docs agree"
        );
    }
}

#[test]
fn the_inventory_names_every_current_version_claim_in_the_tracked_tree() {
    // Historical records are not current-version claims: the changelog, the
    // completed bootstrap in RELEASING.md, and DESIGN.md's roadmap may quote
    // any release they like.
    const HISTORY: &[&str] = &["CHANGELOG.md", "RELEASING.md", "DESIGN.md"];

    let root = animsmith_testkit::repo_root();
    let tracked = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["ls-files", "*.md"])
        .output()
        .expect("lists tracked Markdown");
    assert!(tracked.status.success(), "git ls-files must succeed");
    let tracked = String::from_utf8(tracked.stdout).expect("tracked paths are UTF-8");
    let tracked: Vec<_> = tracked
        .lines()
        .filter(|path| !HISTORY.contains(path))
        .collect();
    assert!(
        tracked.len() > 50,
        "the scan must see the tracked documentation, found {} files",
        tracked.len()
    );

    // Read independently of the module under test: a line that *is* a Cargo
    // requirement for an animsmith package, and a `"tool"` object naming
    // animsmith. If either shape appears outside the inventory, or in
    // different numbers, the inventory no longer describes the tree.
    let mut scanned: BTreeMap<String, (Vec<String>, usize)> = BTreeMap::new();
    for path in tracked {
        let content = std::fs::read_to_string(root.join(path)).expect("reads tracked Markdown");
        let mut packages = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            let Some((package, requirement)) = line.split_once(" = \"") else {
                continue;
            };
            let is_requirement = requirement
                .strip_suffix('"')
                .is_some_and(|requirement| requirement.split('.').count() == 2);
            if package.starts_with("animsmith-") && is_requirement {
                packages.push(package.to_owned());
            }
        }
        let tools = content
            .match_indices("\"tool\"")
            .filter(|(index, _)| {
                let rest = content[index + "\"tool\"".len()..].trim_start();
                let Some(object) = rest.strip_prefix(':') else {
                    return false;
                };
                let object = object.trim_start();
                object.starts_with('{') && object[..object.len().min(400)].contains("\"animsmith\"")
            })
            .count();
        if !packages.is_empty() || tools > 0 {
            scanned.insert(path.to_owned(), (packages, tools));
        }
    }

    let inventoried: BTreeMap<String, (Vec<String>, usize)> = INVENTORY
        .iter()
        .map(|document| {
            (
                document.path.to_owned(),
                (
                    document
                        .packages
                        .iter()
                        .map(|package| (*package).to_owned())
                        .collect(),
                    document.tool_examples,
                ),
            )
        })
        .collect();
    assert_eq!(
        scanned, inventoried,
        "every current-version claim in the tracked tree is inventoried, with its packages and example count"
    );
}

#[test]
fn the_inventory_names_the_documents_the_acceptance_criteria_name() {
    let dependency_paths: BTreeSet<_> = INVENTORY
        .iter()
        .filter(|document| !document.packages.is_empty())
        .map(|document| document.path)
        .collect();
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
    let tool_documents: Vec<_> = INVENTORY
        .iter()
        .filter(|document| document.tool_examples > 0)
        .map(|document| (document.path, document.tool_examples))
        .collect();
    assert_eq!(
        tool_documents,
        vec![
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
fn every_release_context_signal_selects_its_mode() {
    let variable = |set: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            set.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    };
    let none = variable(&[]);

    for explicit in ["1", "true"] {
        let set: &'static [(&'static str, &'static str)] = match explicit {
            "1" => &[("ANIMSMITH_RELEASE_PR", "1")],
            _ => &[("ANIMSMITH_RELEASE_PR", "true")],
        };
        assert_eq!(
            versions::release_mode(variable(set), Some("main")),
            ReleaseMode::ReleasePr,
            "an explicit {explicit:?} selects release-PR strictness"
        );
    }
    assert_eq!(
        versions::release_mode(
            variable(&[
                ("ANIMSMITH_RELEASE_PR", "false"),
                ("GITHUB_HEAD_REF", "release-plz-2026-08-16"),
            ]),
            Some("release-plz-2026-08-16")
        ),
        ReleaseMode::Staging,
        "an explicit false override keeps local diagnostic runs non-strict"
    );
    assert_eq!(
        versions::release_mode(
            variable(&[("GITHUB_HEAD_REF", "release-plz-2026-08-16")]),
            Some("main")
        ),
        ReleaseMode::ReleasePr,
        "CI exports the generated branch as the pull-request head ref"
    );
    assert_eq!(
        versions::release_mode(
            variable(&[("ANIMSMITH_RELEASE_PR", "release-plz-2026-08-16")]),
            Some("main")
        ),
        ReleaseMode::Staging,
        "the head ref is read from GITHUB_HEAD_REF, not from the explicit override"
    );
    assert_eq!(
        versions::release_mode(none, Some("release-plz-2026-08-16")),
        ReleaseMode::ReleasePr,
        "a checked-out generated branch is strict without any CI variable"
    );
    assert_eq!(
        versions::release_mode(
            variable(&[("GITHUB_HEAD_REF", "feature/docs")]),
            Some("main")
        ),
        ReleaseMode::Staging,
        "an ordinary branch stages"
    );

    // The git leg: a checkout is read through `git branch --show-current`.
    for (branch, expected) in [
        ("release-plz-2026-08-16", ReleaseMode::ReleasePr),
        ("main", ReleaseMode::Staging),
    ] {
        let checkout = tempfile::tempdir().expect("temporary checkout");
        for arguments in [
            vec!["init", "--quiet", "--initial-branch", branch],
            vec!["config", "user.email", "gate@example.invalid"],
            vec!["config", "user.name", "gate"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "root"],
        ] {
            let status = Command::new("git")
                .arg("-C")
                .arg(checkout.path())
                .args(&arguments)
                .status()
                .expect("runs git");
            assert!(status.success(), "git {arguments:?} must succeed");
        }
        assert_eq!(
            versions::current_branch(checkout.path()).as_deref(),
            Some(branch),
            "the checked-out branch is what git reports"
        );
        let branch_of = versions::current_branch(checkout.path());
        assert_eq!(
            versions::release_mode(none, branch_of.as_deref()),
            expected,
            "a checkout on {branch} with no release variable set is {expected:?}"
        );
        assert_eq!(
            versions::release_mode(
                variable(&[("ANIMSMITH_RELEASE_PR", "true")]),
                branch_of.as_deref()
            ),
            ReleaseMode::ReleasePr,
            "an explicit release-PR run is strict on a {branch} checkout"
        );
    }
}

#[test]
fn successor_policy_rejects_two_patches_ahead_and_cross_domain_drift() {
    let root = animsmith_testkit::repo_root();
    let workspace = versions::workspace_version(&root).expect("reads workspace version");
    let documented = documented_release();
    let original = repository_docs();

    let two_patches = Version {
        patch: workspace.patch + 2,
        ..workspace
    };
    let mut too_far = original.clone();
    replace_all(
        &mut too_far,
        &format!("\"version\": \"{documented}\""),
        &format!("\"version\": \"{two_patches}\""),
    );
    assert!(
        versions::validate(workspace, &too_far, ReleaseMode::Staging)
            .iter()
            .any(|error| error.contains("current docs describe")),
        "ordinary main may not stage two patch releases ahead"
    );

    // Each half is individually inside the window, but together they name
    // two releases.
    let mut crossed = original;
    replace_all(
        &mut crossed,
        &format!(" = \"{}\"", documented.dependency_line()),
        &format!(" = \"{}\"", workspace.next_minor().dependency_line()),
    );
    replace_all(
        &mut crossed,
        &format!("\"version\": \"{documented}\""),
        &format!("\"version\": \"{}\"", workspace.next_patch()),
    );
    assert!(
        versions::validate(workspace, &crossed, ReleaseMode::Staging)
            .iter()
            .any(|error| error.contains("dependency requirement states")),
        "individually allowed successors must still describe one release line"
    );
}

#[test]
fn malformed_current_tool_json_is_rejected() {
    let root = animsmith_testkit::repo_root();
    let workspace = versions::workspace_version(&root).expect("reads workspace version");
    let mut docs = repository_docs();
    let content = docs.get_mut("docs/mixamo-tutorial.md").expect("tutorial");
    *content = content.replacen(
        "\"name\": \"animsmith\"",
        "\"name\": \"animsmith\", INVALID",
        1,
    );
    assert!(
        versions::validate(workspace, &docs, ReleaseMode::Staging)
            .iter()
            .any(|error| error.contains("parses current `tool` example")),
        "the gate must parse the complete tool object rather than extract version text"
    );
}

#[test]
fn every_stale_dependency_and_tool_version_mutation_fails() {
    let root = animsmith_testkit::repo_root();
    let workspace = versions::workspace_version(&root).expect("reads workspace version");
    let documented = documented_release();
    let docs = repository_docs();

    for document in INVENTORY {
        for &package in document.packages {
            let from = format!("{package} = \"{}\"", documented.dependency_line());
            for stale in ["0.0", "999.999"] {
                let mut mutated = docs.clone();
                let content = mutated.get_mut(document.path).unwrap();
                *content = replace_nth(content, &from, &format!("{package} = \"{stale}\""), 0);
                let errors = versions::validate(workspace, &mutated, ReleaseMode::Staging);
                assert!(
                    errors
                        .iter()
                        .any(|error| error.contains(document.path) && error.contains(package)),
                    "stale {package} dependency {stale} in {} must fail: {errors:?}",
                    document.path
                );
            }
        }

        let from = format!("\"version\": \"{documented}\"");
        for occurrence in 0..document.tool_examples {
            for stale in ["0.0.0", "999.999.999"] {
                let mut mutated = docs.clone();
                let content = mutated.get_mut(document.path).unwrap();
                *content = replace_nth(
                    content,
                    &from,
                    &format!("\"version\": \"{stale}\""),
                    occurrence,
                );
                let errors = versions::validate(workspace, &mutated, ReleaseMode::Staging);
                assert!(
                    errors.iter().any(
                        |error| error.contains(document.path) && error.contains("tool.version")
                    ),
                    "stale tool.version {stale} occurrence {occurrence} in {} must fail: {errors:?}",
                    document.path
                );
            }
        }
    }
}

#[test]
fn a_manifest_bump_restates_every_claim_and_leaves_the_stale_copy_failing() {
    let documented = documented_release();
    // A minor bump moves both claim spellings; a patch bump would leave the
    // `X.Y` dependency requirements alone.
    let bumped = documented.next_minor();
    let fixture = Fixture::new(bumped);
    let stale = fixture.snapshot();

    let changes = versions::stage_release_docs(fixture.root(), bumped).expect("stages the bump");
    assert_eq!(
        changes.len(),
        inventoried_claims(),
        "a manifest bump restates every inventoried claim: {changes:?}"
    );
    let restated: BTreeSet<_> = changes.iter().map(|change| change.path).collect();
    let inventoried: BTreeSet<_> = INVENTORY.iter().map(|document| document.path).collect();
    assert_eq!(
        restated, inventoried,
        "every inventoried document is written"
    );
    for change in &changes {
        assert_eq!(
            change.to,
            change.kind.render(bumped),
            "{change} states the bumped version"
        );
    }

    let staged = fixture.snapshot();
    assert_eq!(
        versions::validate(bumped, &staged, ReleaseMode::ReleasePr),
        Vec::<String>::new(),
        "the release PR accepts what the writer wrote"
    );
    assert!(
        versions::validate(bumped, &stale, ReleaseMode::ReleasePr)
            .iter()
            .any(|error| error.contains(&format!("Cargo.toml releases {bumped}"))),
        "the release PR rejects the pre-generation copy against the bumped manifest"
    );
    assert!(
        versions::validate(bumped, &stale, ReleaseMode::Staging)
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
    let documented = documented_release();
    let bumped = documented.next_minor();
    let fixture = Fixture::new(bumped);

    // Prose quoting the version being replaced, in the shapes a
    // whole-document search-and-replace would rewrite: the documented
    // dependency requirement, a documented tool version in a JSON object
    // that is not a `tool` object, and the release's own name.
    let history = format!(
        "\n## History (fixture)\n\nAnimSmith {documented} is the release this page was written \
         for. Its manifests read `animsmith-core = \"{line}\"`, and a report from the {line} \
         line carried `\"generator\": {{ \"name\": \"animsmith\", \"version\": \"{documented}\" }}`.\n",
        line = documented.dependency_line(),
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
        versions::validate(bumped, &fixture.snapshot(), ReleaseMode::ReleasePr),
        Vec::<String>::new(),
        "a historical paragraph is not a current-version claim"
    );
}

#[test]
fn a_nested_version_belongs_to_its_own_object() {
    let documented = documented_release();
    let bumped = documented.next_minor();
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
        versions::validate(bumped, &fixture.snapshot(), ReleaseMode::ReleasePr),
        Vec::<String>::new(),
        "the tool object still states one release"
    );
}

#[test]
fn staging_refuses_a_version_outside_the_release_window() {
    // The fixture is in the released state: its manifest is the release its
    // documents describe, whether or not the repository is mid-staging.
    let documented = documented_release();
    let fixture = Fixture::new(documented);
    let before = fixture.snapshot();

    let mut refused = vec![
        Version {
            patch: documented.patch + 2,
            ..documented
        },
        documented.next_minor().next_minor(),
        Version {
            major: documented.major + 1,
            minor: 0,
            patch: 0,
        },
    ];
    // Backwards is outside the window too: the window is the release the
    // manifest carries and its successors, not everything near it.
    let mut backwards = Vec::new();
    if documented.patch > 0 {
        backwards.push(Version {
            patch: documented.patch - 1,
            ..documented
        });
    }
    if documented.minor > 0 {
        backwards.push(Version {
            minor: documented.minor - 1,
            patch: 0,
            ..documented
        });
    }
    if documented.major > 0 {
        backwards.push(Version {
            major: documented.major - 1,
            minor: 0,
            patch: 0,
        });
    }
    assert!(
        !backwards.is_empty(),
        "the released version {documented} has a predecessor the window must refuse"
    );
    refused.append(&mut backwards);

    for refused in refused {
        let error = versions::stage_release_docs(fixture.root(), refused)
            .expect_err("a version outside the release window is refused");
        assert!(
            error.contains(&refused.to_string()) && error.contains(&documented.to_string()),
            "the refusal names the requested version and the window: {error}"
        );
        assert_eq!(
            fixture.snapshot(),
            before,
            "a refused version writes nothing"
        );
    }

    for accepted in documented.release_window() {
        versions::stage_release_docs(fixture.root(), accepted)
            .expect("the manifest version and its next patch and minor are stageable");
        let staged = fixture.snapshot();
        assert_eq!(
            versions::validate(documented, &staged, ReleaseMode::Staging),
            Vec::<String>::new(),
            "what the writer staged at {accepted} validates against the manifest {documented}"
        );
        assert_eq!(
            versions::validate(accepted, &staged, ReleaseMode::ReleasePr),
            Vec::<String>::new(),
            "and passes the release PR whose manifest is {accepted}"
        );
        versions::stage_release_docs(fixture.root(), documented).expect("restores the fixture");
    }
    assert_eq!(
        fixture.snapshot(),
        before,
        "staging back to the released version restores the documents"
    );
}

#[test]
fn an_unreadable_document_stops_the_writer_before_it_writes_anything() {
    let documented = documented_release();

    for (path, break_it, expected) in [
        (
            "docs/embedding.md",
            Box::new(|content: String| {
                content.replacen(
                    "animsmith-core = \"",
                    "animsmith-core = \"0.1\"\nanimsmith-core = \"",
                    1,
                )
            }) as Box<dyn Fn(String) -> String>,
            "expected exactly one current `animsmith-core = \"X.Y\"` snippet, found 2",
        ),
        (
            "crates/animsmith-report/README.md",
            Box::new(|content: String| {
                content.replacen("animsmith-report = \"", "animsmith-reportx = \"", 1)
            }),
            "expected exactly one current `animsmith-report = \"X.Y\"` snippet, found 0",
        ),
        (
            "docs/output.md",
            Box::new(|content: String| {
                format!(
                    "{content}\n```json\n{{ \"tool\": {{ \"name\": \"animsmith\", \"version\": \"{documented}\" }} }}\n```\n"
                )
            }),
            "expected 4 current `tool.version` example(s), found 5",
        ),
    ] {
        let fixture = Fixture::new(documented);
        fixture.write(path, &break_it(fixture.read(path)));
        let before = fixture.snapshot();

        let errors = versions::validate(documented, &before, ReleaseMode::Staging);
        assert!(
            errors.iter().any(|error| error.contains(expected)),
            "{path}: the gate reports {expected:?}: {errors:?}"
        );

        let error = versions::stage_release_docs(fixture.root(), documented.next_minor())
            .expect_err("an unreadable document is not partially rewritten");
        assert!(
            error.contains(expected),
            "{path}: the writer refuses with the same reason: {error}"
        );
        assert_eq!(
            fixture.snapshot(),
            before,
            "{path}: no document is written when one cannot be read"
        );
    }
}

#[test]
fn the_staging_tool_takes_one_argument_and_refuses_every_other_invocation() {
    let owned = |arguments: &[&str]| {
        arguments
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect::<Vec<_>>()
    };

    assert_eq!(
        versions::requested_version(owned(&[])),
        Ok(None),
        "no argument leaves the target to the workspace manifest"
    );
    assert_eq!(
        versions::requested_version(owned(&["--version", "1.2.3"])),
        Ok(Some(Version {
            major: 1,
            minor: 2,
            patch: 3
        })),
        "`--version X.Y.Z` names the release line to stage"
    );

    for refused in [
        vec!["1.2.3"],
        vec!["--target", "1.2.3"],
        vec!["--version"],
        vec!["--version", "1.2"],
        vec!["--version", "v1.2.3"],
        vec!["--version", "1.2.3", "0.1.0"],
    ] {
        let error = versions::requested_version(owned(&refused))
            .expect_err(&format!("{refused:?} is not a valid invocation"));
        assert!(
            error.contains(STAGE_COMMAND),
            "the refusal of {refused:?} shows the usage: {error}"
        );
    }
}

/// Build the `stage_release_docs` example and answer where it landed.
///
/// Cargo puts an example beside the test binary's own profile directory —
/// `<target>/<profile>/examples/<example>` next to
/// `<target>/<profile>/deps/<test>` — so this test's path names the target
/// directory and the profile to build into, and the binary it finds there
/// is the one the documented `cargo run` would have used.
fn staging_example() -> PathBuf {
    let test = std::env::current_exe().expect("the test binary's path");
    let profile_dir = test
        .parent()
        .and_then(Path::parent)
        .expect("the test does not sit in <target>/<profile>/deps");
    let profile = profile_dir
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("the build profile directory has no name");
    let target_dir = profile_dir.parent().expect("the target directory");

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| std::ffi::OsString::from("cargo"));
    let mut build = Command::new(cargo);
    build
        .args(["build", "--quiet", "-p", "animsmith"])
        .args(["--example", "stage_release_docs"])
        .arg("--target-dir")
        .arg(target_dir)
        .current_dir(animsmith_testkit::repo_root());
    // Cargo names the dev profile's directory `debug`; every other
    // profile's directory carries the profile's own name.
    if profile != "debug" {
        build.args(["--profile", profile]);
    }
    let status = build.status().expect("runs cargo build --example");
    assert!(status.success(), "cargo build --example must succeed");

    let example = profile_dir.join("examples").join(format!(
        "stage_release_docs{}",
        std::env::consts::EXE_SUFFIX
    ));
    assert!(example.is_file(), "{} was not built", example.display());
    example
}

#[test]
fn the_staging_example_stages_the_next_release_restores_it_and_refuses_a_bad_invocation() {
    let documented = documented_release();
    let fixture = Fixture::new(documented);
    let before = fixture.snapshot();
    let example = staging_example();
    let next = documented.next_minor();

    let run = |arguments: &[&str]| {
        Command::new(&example)
            .args(arguments)
            .env("ANIMSMITH_DOCS_ROOT", fixture.root())
            .env_remove("ANIMSMITH_RELEASE_PR")
            .output()
            .expect("runs the staging example")
    };

    let staged = run(&["--version", &next.to_string()]);
    assert!(
        staged.status.success(),
        "staging the next release exits 0: {staged:?}"
    );
    let report = String::from_utf8(staged.stdout).expect("the report is UTF-8");
    assert!(
        report.contains(&format!(
            "staged {} current-version claim(s) at {next}",
            inventoried_claims()
        )),
        "the example reports what it staged: {report}"
    );
    assert_eq!(
        versions::validate(next, &fixture.snapshot(), ReleaseMode::ReleasePr),
        Vec::<String>::new(),
        "the example staged the release it was asked for"
    );
    assert_ne!(fixture.snapshot(), before, "the documents moved");

    let restored = run(&[]);
    assert!(
        restored.status.success(),
        "restating the manifest version exits 0: {restored:?}"
    );
    assert_eq!(
        fixture.snapshot(),
        before,
        "with no argument the example writes the version the manifest releases"
    );

    let again = run(&[]);
    assert!(again.status.success(), "a second run exits 0");
    assert!(
        String::from_utf8_lossy(&again.stdout).contains(&format!("already describes {documented}")),
        "a second run reports that nothing moved"
    );

    let refused = run(&["--oops"]);
    assert!(
        !refused.status.success(),
        "a bad invocation exits non-zero: {refused:?}"
    );
    let complaint = String::from_utf8(refused.stderr).expect("the complaint is UTF-8");
    assert!(
        complaint.contains("unexpected argument") && complaint.contains(STAGE_COMMAND),
        "a bad invocation prints the usage: {complaint}"
    );
    assert_eq!(
        fixture.snapshot(),
        before,
        "a bad invocation writes nothing"
    );
}

#[test]
fn the_staging_example_refuses_a_root_override_that_names_the_repository() {
    let repository = animsmith_testkit::repo_root();
    let example = staging_example();
    let before = repository_docs();

    // Every spelling of the repository, because the override exists only to
    // send the writer somewhere else: a stale or dropped value must fail
    // loudly rather than quietly rewrite the tree the test left alone.
    for overridden in [
        std::fs::canonicalize(&repository).expect("the repository resolves"),
        repository.join("crates").join(".."),
        repository.clone(),
    ] {
        let refused = Command::new(&example)
            .env("ANIMSMITH_DOCS_ROOT", &overridden)
            .env_remove("ANIMSMITH_RELEASE_PR")
            .output()
            .expect("runs the staging example");
        assert!(
            !refused.status.success(),
            "{} is the repository, so the example refuses: {refused:?}",
            overridden.display()
        );
        let complaint = String::from_utf8(refused.stderr).expect("the complaint is UTF-8");
        assert!(
            complaint.contains("ANIMSMITH_DOCS_ROOT"),
            "the refusal names the override: {complaint}"
        );
        assert_eq!(
            repository_docs(),
            before,
            "{}: the repository's own documents are untouched",
            overridden.display()
        );
    }
}

/// The inventory is a table of `Document`s; nothing else in the gate may
/// assume its shape.
#[test]
fn the_inventory_is_one_table() {
    let documents: Vec<&Document> = INVENTORY.iter().collect();
    assert_eq!(
        documents.len(),
        documents
            .iter()
            .map(|document| document.path)
            .collect::<BTreeSet<_>>()
            .len(),
        "each document appears once"
    );
    assert!(
        documents
            .iter()
            .all(|document| !document.packages.is_empty() || document.tool_examples > 0),
        "a document in the inventory states something"
    );
}
