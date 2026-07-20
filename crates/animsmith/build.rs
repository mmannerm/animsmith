use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    watch_git_metadata();

    let package_version = env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");
    let version = resolved_version(&package_version, git_version(&package_version));

    println!("cargo:rustc-env=ANIMSMITH_VERSION={version}");
    if let Some(source) = source_info() {
        println!("cargo:rustc-env=ANIMSMITH_GIT_REVISION={}", source.revision);
        if let Some(dirty) = source.dirty {
            println!("cargo:rustc-env=ANIMSMITH_GIT_DIRTY={dirty}");
        }
    }
}

pub(crate) fn resolved_version(package_version: &str, git_version: Option<String>) -> String {
    git_version.unwrap_or_else(|| package_version.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceInfo {
    pub(crate) revision: String,
    pub(crate) dirty: Option<bool>,
}

fn source_info() -> Option<SourceInfo> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    let packaged = manifest_dir.join(".cargo_vcs_info.json");
    if packaged.is_file() {
        return packaged_source_info(&packaged);
    }
    let git_root = trusted_git_root()?;
    let revision = successful_git_text(&git_root, ["rev-parse", "HEAD"])?;
    let status = git(&git_root, ["status", "--porcelain", "--untracked-files=no"])?;
    if !status.status.success() {
        return None;
    }
    Some(SourceInfo {
        revision,
        dirty: Some(!status.stdout.is_empty()),
    })
}

pub(crate) fn packaged_source_info(path: &Path) -> Option<SourceInfo> {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    let revision = value.get("git")?.get("sha1")?.as_str()?.to_owned();
    Some(SourceInfo {
        revision,
        dirty: None,
    })
}

fn git_version(package_version: &str) -> Option<String> {
    let git_root = trusted_git_root()?;
    let describe = git_describe(&git_root)?;
    Some(display_version(package_version, &describe))
}

fn git_describe(git_root: &Path) -> Option<String> {
    successful_git_text(git_root, ["describe", "--tags", "--dirty", "--always"])
}

fn successful_git_text<const N: usize>(git_root: &Path, args: [&str; N]) -> Option<String> {
    let output = git(git_root, args)?;
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

    // Dirty state is part of the machine-readable source identity. Cargo
    // otherwise would not rerun this build script for an unstaged edit, so
    // watch every tracked file whose state contributes to that bit.
    let Some(output) = git(&git_root, ["ls-files"]) else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let Ok(files) = String::from_utf8(output.stdout) else {
        return;
    };
    for file in files.lines().filter(|file| !file.is_empty()) {
        println!("cargo:rerun-if-changed={}", git_root.join(file).display());
    }
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
    let exact_tag = format!("v{package_version}");
    if describe == package_version || describe == exact_tag {
        package_version.to_owned()
    } else {
        format!("{package_version} ({describe})")
    }
}
