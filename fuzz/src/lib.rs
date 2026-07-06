//! Shared harness helpers for the fuzz targets.
//!
//! The loaders' public entry points (`animsmith_gltf::load`,
//! `animsmith_gltf::fix::fix_quat_hemisphere`, `animsmith_fbx::load`) take
//! a `&Path`, so each target has to materialize its fuzz bytes on disk
//! before calling in. We keep one scratch directory per process and reuse
//! fixed filenames inside it, so file churn stays negligible next to the
//! parse work being exercised.
//!
//! Keeping the directory isolated and otherwise empty also bounds the
//! blast radius of glTF external-buffer URIs: a URI that resolves to a
//! sibling path finds nothing and yields a `LoadError` rather than reading
//! unrelated files.

use std::path::PathBuf;
use std::sync::OnceLock;

fn scratch_dir() -> &'static tempfile::TempDir {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| tempfile::tempdir().expect("create fuzz scratch dir"))
}

/// Write `data` to a stable per-process scratch file named `name` and
/// return its path. Panics only on harness I/O failure, never on input.
pub fn scratch_file(name: &str, data: &[u8]) -> PathBuf {
    let path = scratch_dir().path().join(name);
    std::fs::write(&path, data).expect("write fuzz scratch file");
    path
}

/// A scratch path inside the per-process directory without writing to it
/// (for a target that needs a distinct output file alongside its input).
pub fn scratch_path(name: &str) -> PathBuf {
    scratch_dir().path().join(name)
}
