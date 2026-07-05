use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    watch_git_metadata();

    let package_version = env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");
    let version = git_version(&package_version).unwrap_or(package_version);

    println!("cargo:rustc-env=ANIMSMITH_VERSION={version}");
}

fn git_version(package_version: &str) -> Option<String> {
    let git_root = trusted_git_root()?;
    let describe = git_describe(&git_root)?;
    Some(display_version(package_version, &describe))
}

fn git_describe(git_root: &Path) -> Option<String> {
    let output = git(git_root, ["describe", "--tags", "--dirty", "--always"])?;
    if !output.status.success() {
        return None;
    }
    let describe = String::from_utf8(output.stdout).ok()?;
    let describe = describe.trim();
    (!describe.is_empty()).then(|| describe.to_owned())
}

fn watch_git_metadata() {
    let Some(git_root) = trusted_git_root() else {
        return;
    };
    let Some(output) = git(&git_root, ["rev-parse", "--git-dir"]) else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let Ok(git_dir) = String::from_utf8(output.stdout) else {
        return;
    };
    let git_dir = git_dir.trim();
    if git_dir.is_empty() {
        return;
    }
    println!("cargo:rerun-if-changed={git_dir}/HEAD");
    println!("cargo:rerun-if-changed={git_dir}/index");
    println!("cargo:rerun-if-changed={git_dir}/refs/tags");
}

fn trusted_git_root() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);

    let output = git(&manifest_dir, ["rev-parse", "--show-toplevel"])?;
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8(output.stdout).ok()?;
    let root = PathBuf::from(root.trim());
    if root.as_os_str().is_empty() {
        return None;
    }

    trusted_git_root_for_manifest(&manifest_dir, &root)
}

pub(crate) fn trusted_git_root_for_manifest(
    manifest_dir: &Path,
    git_root: &Path,
) -> Option<PathBuf> {
    // Cargo package builds include vcs metadata separately; do not look
    // through the package directory into any surrounding repository.
    if manifest_dir.join(".cargo_vcs_info.json").is_file() {
        return None;
    }

    let expected_manifest = git_root.join("crates").join("animsmith").join("Cargo.toml");
    let actual_manifest = manifest_dir.join("Cargo.toml");
    let expected_manifest = expected_manifest.canonicalize().ok()?;
    let actual_manifest = actual_manifest.canonicalize().ok()?;
    (expected_manifest == actual_manifest).then(|| git_root.to_owned())
}

fn git<const N: usize>(dir: &Path, args: [&str; N]) -> Option<std::process::Output> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()
}

pub(crate) fn display_version(package_version: &str, describe: &str) -> String {
    format!("{package_version} ({describe})")
}
