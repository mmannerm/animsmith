use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=ANIMSMITH_BUILD_VERSION");
    println!("cargo:rerun-if-changed=build.rs");
    watch_git_metadata();

    let package_version = env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");
    let version = env::var("ANIMSMITH_BUILD_VERSION")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| git_version(&package_version))
        .unwrap_or(package_version);

    println!("cargo:rustc-env=ANIMSMITH_VERSION={version}");
}

fn git_version(package_version: &str) -> Option<String> {
    let describe = git_describe()?;
    if describe_matches_package_version(package_version, &describe) {
        Some(display_version(package_version, &describe))
    } else {
        git_revision().map(|revision| format!("{package_version} ({revision})"))
    }
}

fn git_describe() -> Option<String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let output = git(&manifest_dir, ["describe", "--tags", "--dirty", "--always"])?;
    if !output.status.success() {
        return None;
    }
    let describe = String::from_utf8(output.stdout).ok()?;
    let describe = describe.trim();
    (!describe.is_empty()).then(|| describe.to_owned())
}

fn git_revision() -> Option<String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let output = git(&manifest_dir, ["rev-parse", "--short", "HEAD"])?;
    if !output.status.success() {
        return None;
    }
    let revision = String::from_utf8(output.stdout).ok()?;
    let mut revision = revision.trim().to_owned();
    if revision.is_empty() {
        return None;
    }
    if is_dirty(&manifest_dir) {
        revision.push_str("-dirty");
    }
    Some(revision)
}

fn is_dirty(manifest_dir: &str) -> bool {
    git(manifest_dir, ["diff-index", "--quiet", "HEAD", "--"])
        .is_some_and(|output| !output.status.success())
}

fn watch_git_metadata() {
    let Some(manifest_dir) = env::var("CARGO_MANIFEST_DIR").ok() else {
        return;
    };
    let Some(output) = git(&manifest_dir, ["rev-parse", "--git-dir"]) else {
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

fn git<const N: usize>(manifest_dir: &str, args: [&str; N]) -> Option<std::process::Output> {
    Command::new("git")
        .arg("-C")
        .arg(manifest_dir)
        .args(args)
        .output()
        .ok()
}

fn display_version(package_version: &str, describe: &str) -> String {
    let exact_tag = format!("v{package_version}");
    if describe == package_version || describe == exact_tag {
        package_version.to_owned()
    } else {
        format!("{package_version} ({describe})")
    }
}

fn describe_matches_package_version(package_version: &str, describe: &str) -> bool {
    describe == package_version
        || describe == format!("v{package_version}")
        || describe.starts_with(&format!("{package_version}-"))
        || describe.starts_with(&format!("v{package_version}-"))
}
