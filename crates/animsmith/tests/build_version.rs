#[allow(dead_code)]
#[path = "../build.rs"]
mod build_script;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
fn trusted_git_root_accepts_animsmith_workspace_layout() {
    let temp = TempDir::new("workspace-layout");
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
    let temp = TempDir::new("foreign-layout");
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
    let temp = TempDir::new("cargo-package");
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

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "animsmith-build-version-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("creates temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
