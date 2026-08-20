#![cfg(unix)]

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const CLI_MANIFEST: &str = r#"[package]
name = "animsmith"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "animsmith"
path = "src/main.rs"
"#;

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().expect("fixture path has a parent"))
        .expect("create fixture directory");
    fs::write(path, contents).expect("write fixture file");
}

fn fixture() -> TempDir {
    let fixture = tempfile::tempdir().expect("create fixture workspace");
    let root = fixture.path();
    write(
        root.join("Cargo.toml"),
        r#"[workspace]
members = [
    "crates/library",
    "crates/animsmith",
]
resolver = "3"
"#,
    );
    write(
        root.join("crates/library/Cargo.toml"),
        r#"[package]
name = "library"
version = "0.1.0"
edition = "2024"
"#,
    );
    write(root.join("crates/library/src/lib.rs"), "");
    write(root.join("crates/animsmith/Cargo.toml"), CLI_MANIFEST);
    write(root.join("crates/animsmith/src/main.rs"), "fn main() {}\n");

    let source_script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/check-package-inventory.sh");
    fs::create_dir(root.join("scripts")).expect("create scripts directory");
    fs::copy(
        source_script,
        root.join("scripts/check-package-inventory.sh"),
    )
    .expect("copy package policy script");
    fixture
}

fn run(fixture: &TempDir) -> Output {
    Command::new("bash")
        .arg("scripts/check-package-inventory.sh")
        .current_dir(fixture.path())
        .env("ANIMSMITH_PACKAGE_INVENTORY_TARGET_POLICY_ONLY", "1")
        .output()
        .expect("run package target policy")
}

fn assert_rejected(label: &str, mutate: impl FnOnce(&Path), expected_stderr: &str) {
    let fixture = fixture();
    mutate(fixture.path());
    let output = run(&fixture);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "{label} unexpectedly passed: {stderr}"
    );
    assert!(
        stderr.contains(expected_stderr),
        "{label} emitted the wrong refusal; expected {expected_stderr:?}, got {stderr:?}"
    );
}

#[test]
fn explicit_bin_only_policy_accepts_the_cli_and_library_contract() {
    let fixture = fixture();
    let output = run(&fixture);
    assert!(
        output.status.success(),
        "valid policy fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn target_paths_match_when_the_checkout_is_reached_through_a_symlink() {
    let fixture = fixture();
    let link_parent = tempfile::tempdir().expect("create symlink parent");
    let linked_workspace = link_parent.path().join("workspace");
    symlink(fixture.path(), &linked_workspace).expect("link fixture workspace");

    let output = Command::new("bash")
        .arg("-c")
        .arg(
            "cd \"$1\" && ANIMSMITH_PACKAGE_INVENTORY_TARGET_POLICY_ONLY=1 \
             bash scripts/check-package-inventory.sh",
        )
        .arg("package-inventory-policy")
        .arg(&linked_workspace)
        .output()
        .expect("run package target policy through symlink");
    assert!(
        output.status.success(),
        "symlinked policy fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn every_target_shape_mutation_is_rejected() {
    assert_rejected(
        "missing CLI source",
        |root| fs::remove_file(root.join("crates/animsmith/src/main.rs")).expect("remove main"),
        "only when its bin source exists",
    );
    assert_rejected(
        "implicit binary",
        |root| {
            write(
                root.join("crates/animsmith/Cargo.toml"),
                &CLI_MANIFEST[..CLI_MANIFEST.find("[[bin]]").expect("bin table")],
            )
        },
        "requires an explicit [[bin]] target",
    );
    assert_rejected(
        "redirected binary",
        |root| {
            write(root.join("crates/animsmith/src/other.rs"), "fn main() {}\n");
            write(
                root.join("crates/animsmith/Cargo.toml"),
                &CLI_MANIFEST.replace("src/main.rs", "src/other.rs"),
            );
        },
        "bin-only target must use crates/animsmith/src/main.rs",
    );
    assert_rejected(
        "implicit CLI library",
        |root| write(root.join("crates/animsmith/src/lib.rs"), ""),
        "must not have a library source",
    );
    assert_rejected(
        "explicit redirected CLI library",
        |root| {
            write(root.join("crates/animsmith/src/other_lib.rs"), "");
            let manifest = format!("{CLI_MANIFEST}\n[lib]\npath = \"src/other_lib.rs\"\n");
            write(root.join("crates/animsmith/Cargo.toml"), &manifest);
        },
        "must not have a Cargo library target",
    );
    assert_rejected(
        "second CLI binary",
        |root| {
            write(root.join("crates/animsmith/src/other.rs"), "fn main() {}\n");
            let manifest =
                format!("{CLI_MANIFEST}\n[[bin]]\nname = \"other\"\npath = \"src/other.rs\"\n");
            write(root.join("crates/animsmith/Cargo.toml"), &manifest);
        },
        "requires exactly one Cargo binary target",
    );
    assert_rejected(
        "redirected library",
        |root| {
            write(root.join("crates/library/src/other.rs"), "");
            write(
                root.join("crates/library/Cargo.toml"),
                r#"[package]
name = "library"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/other.rs"
"#,
            );
        },
        "library target must use crates/library/src/lib.rs",
    );
    assert_rejected(
        "unapproved bin-only package",
        |root| {
            let workspace = fs::read_to_string(root.join("Cargo.toml"))
                .expect("read workspace manifest")
                .replace(
                    "    \"crates/animsmith\",",
                    "    \"crates/animsmith\",\n    \"crates/tool\",",
                );
            write(root.join("Cargo.toml"), &workspace);
            write(
                root.join("crates/tool/Cargo.toml"),
                r#"[package]
name = "tool"
version = "0.1.0"
edition = "2024"
"#,
            );
            write(root.join("crates/tool/src/main.rs"), "fn main() {}\n");
        },
        "must have exactly one Cargo library target or use an explicit bin-only docs.rs exemption",
    );
}
