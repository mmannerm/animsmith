#[allow(dead_code)]
#[path = "../build.rs"]
mod build_script;

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn display_version_appends_git_describe_to_manifest_version() {
    assert_eq!(
        build_script::display_version("0.1.0", "v0.1.0-3-gabc1234"),
        "0.1.0 (v0.1.0-3-gabc1234)"
    );
}

#[test]
fn display_version_suppresses_exact_tag_describe() {
    assert_eq!(build_script::display_version("0.1.0", "0.1.0"), "0.1.0");
    assert_eq!(build_script::display_version("0.1.0", "v0.1.0"), "0.1.0");
}

#[test]
fn display_version_preserves_dirty_non_exact_describe() {
    assert_eq!(
        build_script::display_version("0.1.0", "v0.1.0-dirty"),
        "0.1.0 (v0.1.0-dirty)"
    );
}

#[test]
fn display_version_preserves_bare_commit_describe() {
    assert_eq!(
        build_script::display_version("0.1.0", "abc1234"),
        "0.1.0 (abc1234)"
    );
}

#[test]
fn resolved_version_falls_back_to_the_bare_manifest_version() {
    assert_eq!(build_script::resolved_version("0.1.0", None), "0.1.0");
    assert_eq!(
        build_script::resolved_version("0.1.0", Some("0.1.0 (abc1234)".into())),
        "0.1.0 (abc1234)"
    );
}

#[test]
fn packaged_source_info_reads_full_revision_without_claiming_cleanliness() {
    let temp = temp_dir("cargo-vcs-source");
    let path = temp.path().join(".cargo_vcs_info.json");
    fs::write(
        &path,
        r#"{"git":{"sha1":"0123456789abcdef0123456789abcdef01234567"}}"#,
    )
    .expect("writes vcs info");

    assert_eq!(
        build_script::packaged_source_info(&path),
        Some(build_script::SourceInfo {
            revision: "0123456789abcdef0123456789abcdef01234567".into(),
            dirty: None,
        })
    );
}

#[test]
fn packaged_source_info_rejects_revision_that_cannot_satisfy_the_schema() {
    let temp = temp_dir("invalid-cargo-vcs-source");
    let path = temp.path().join(".cargo_vcs_info.json");
    for revision in [
        "short",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "0123456789abcdef0123456789abcdef0123456z",
        "0123456789abcdef0123456789abcdef01234567\nforge",
    ] {
        fs::write(
            &path,
            serde_json::json!({ "git": { "sha1": revision } }).to_string(),
        )
        .expect("writes vcs info");
        assert_eq!(build_script::packaged_source_info(&path), None);
    }
}

#[test]
fn git_source_info_observes_revision_and_tracked_dirty_state() {
    let temp = temp_dir("git-source");
    git(temp.path(), &["init", "--quiet"]);
    fs::write(temp.path().join("tracked.txt"), "clean\n").expect("writes tracked file");
    git(temp.path(), &["add", "tracked.txt"]);
    git(
        temp.path(),
        &[
            "-c",
            "user.name=animsmith-test",
            "-c",
            "user.email=animsmith@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    );

    let revision = git_output(temp.path(), &["rev-parse", "HEAD"]);
    let clean = build_script::git_source_info(temp.path()).expect("clean source identity");
    assert_eq!(clean.revision, revision);
    assert_eq!(clean.revision.len(), 40);
    assert!(clean.revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(clean.dirty, Some(false));

    fs::write(temp.path().join("tracked.txt"), "dirty\n").expect("edits tracked file");
    let dirty = build_script::git_source_info(temp.path()).expect("dirty source identity");
    assert_eq!(dirty.revision, revision);
    assert_eq!(dirty.dirty, Some(true));
}

#[test]
fn workspace_build_emits_source_identity_environment() {
    let packaged = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cargo_vcs_info.json");
    if packaged.is_file() {
        return;
    }

    let Some(revision) = option_env!("ANIMSMITH_GIT_REVISION") else {
        panic!("Git-worktree build emits ANIMSMITH_GIT_REVISION");
    };
    assert_eq!(revision.len(), 40);
    assert!(revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let Some(dirty) = option_env!("ANIMSMITH_GIT_DIRTY") else {
        panic!("Git-worktree build emits ANIMSMITH_GIT_DIRTY");
    };
    assert!(dirty.parse::<bool>().is_ok(), "dirty identity is a boolean");
}

#[test]
fn trusted_git_root_accepts_animsmith_workspace_layout() {
    let temp = temp_dir("workspace-layout");
    let git_root = temp.path();
    let manifest_dir = git_root.join("crates").join("animsmith");
    write_manifest(&manifest_dir);

    assert_eq!(
        build_script::trusted_git_root_for_manifest(&manifest_dir, git_root),
        Some(git_root.to_owned())
    );
}

#[test]
fn trusted_git_root_rejects_vendored_source_in_foreign_layout() {
    let temp = temp_dir("foreign-layout");
    let git_root = temp.path();
    let trusted_manifest_dir = git_root.join("crates").join("animsmith");
    write_manifest(&trusted_manifest_dir);

    let vendored_manifest_dir = git_root
        .join("vendor")
        .join("animsmith")
        .join("crates")
        .join("animsmith");
    write_manifest(&vendored_manifest_dir);

    let trusted_manifest = trusted_manifest_dir
        .join("Cargo.toml")
        .canonicalize()
        .expect("trusted manifest exists");
    let vendored_manifest = vendored_manifest_dir
        .join("Cargo.toml")
        .canonicalize()
        .expect("vendored manifest exists");
    assert_ne!(
        trusted_manifest, vendored_manifest,
        "fixture must reach the existing-but-different manifest comparison"
    );

    assert_eq!(
        build_script::trusted_git_root_for_manifest(&vendored_manifest_dir, git_root),
        None
    );
}

#[test]
fn trusted_git_root_rejects_packaged_source_with_cargo_vcs_info() {
    let temp = temp_dir("cargo-package");
    let git_root = temp.path();
    let manifest_dir = git_root.join("crates").join("animsmith");
    write_manifest(&manifest_dir);
    fs::write(manifest_dir.join(".cargo_vcs_info.json"), "{}").expect("writes vcs info");

    assert_eq!(
        build_script::trusted_git_root_for_manifest(&manifest_dir, git_root),
        None
    );
}

fn write_manifest(manifest_dir: &Path) {
    fs::create_dir_all(manifest_dir).expect("creates manifest dir");
    fs::write(
        manifest_dir.join("Cargo.toml"),
        "[package]\nname = \"animsmith\"\n",
    )
    .expect("writes manifest");
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("runs git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_output(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("runs git");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8(output.stdout)
        .expect("git output is utf-8")
        .trim()
        .to_owned()
}

fn temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("animsmith-build-version-{name}-"))
        .tempdir()
        .expect("creates temp dir")
}
