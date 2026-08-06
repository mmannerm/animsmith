//! Atomic artifact/evidence publication shared by every producer command.
//!
//! A producer prepares both members of its pair as temporary files beside
//! their destinations, then hands them here. Publication moves any existing
//! destination aside, promotes both temps, and — on any failure — restores
//! whatever was there before, so a run either publishes a complete new pair
//! or leaves the previous one exactly as it found it.
//!
//! This module is deliberately **not** feature gated. `assemble` is the only
//! producer in the default build, but the `scale` producer exists in a
//! `--no-default-features` binary, and both publish the same way.
//!
//! # Durability
//!
//! [`publish_pair`] flushes each temp with `sync_all` before promoting it and
//! then makes a best-effort `fsync` of each destination directory after both
//! renames. Without the first, a crash after the rename can leave a
//! zero-length file where a complete artifact was published; without the
//! second, the rename itself can be lost while the file data survives. The
//! directory flush is best effort because a platform may legitimately refuse
//! to open a directory for `fsync`, and failing publication for that would be
//! worse than publishing without the extra guarantee.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// The directory a path lives in, treating a bare file name as `.`.
pub(crate) fn parent_or_current(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

/// A path's identity for distinctness checks: its canonical parent joined
/// with its own file name.
///
/// Canonicalizing the parent collapses `.`, `..`, and symlinked directories,
/// so two lexically different arguments naming one file compare equal. It
/// does **not** resolve a symlinked final component, so two paths whose last
/// component differs still compare different even if one links to the other.
/// The canonical form exists for this comparison only and is never recorded
/// in evidence: evidence keeps the operator's declared path verbatim.
///
/// # Errors
///
/// Returns an operator error when the parent directory does not exist or
/// cannot be resolved, or when `path` has no file name.
pub(crate) fn destination_identity(path: &Path) -> Result<PathBuf, String> {
    let parent = parent_or_current(path);
    let parent = fs::canonicalize(parent).map_err(|error| {
        format!(
            "cannot resolve output directory {}: {error}",
            parent.display()
        )
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("output {} has no file name", path.display()))?;
    Ok(parent.join(file_name))
}

/// Reject a destination whose directory does not exist, or which exists as
/// something other than a regular file, before anything is prepared for it.
///
/// # Errors
///
/// Returns an operator error naming the destination.
pub(crate) fn require_writable_destination(path: &Path) -> Result<(), String> {
    if !parent_or_current(path).is_dir() {
        return Err(format!(
            "output directory for {} does not exist",
            path.display()
        ));
    }
    if path.exists() && !path.is_file() {
        return Err(format!("output {} is not a regular file", path.display()));
    }
    Ok(())
}

/// Lowercase hex SHA-256 of exactly these bytes.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Read a file back and report its digest and byte count.
///
/// # Errors
///
/// Returns an operator error when the file cannot be read or its length does
/// not fit a `u64`.
pub(crate) fn read_digest(path: &Path) -> Result<(String, u64), String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let size = u64::try_from(bytes.len()).map_err(|_| "input size exceeds u64".to_owned())?;
    Ok((sha256_hex(&bytes), size))
}

fn backup_destination(path: &Path) -> Result<Option<tempfile::TempPath>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let parent = parent_or_current(path);
    let backup = tempfile::Builder::new()
        .prefix(".animsmith-publish-backup-")
        .tempfile_in(parent)
        .map_err(|error| format!("cannot reserve backup for {}: {error}", path.display()))?
        .into_temp_path();
    fs::remove_file(&backup)
        .map_err(|error| format!("cannot prepare backup for {}: {error}", path.display()))?;
    fs::rename(path, &backup)
        .map_err(|error| format!("cannot back up {}: {error}", path.display()))?;
    Ok(Some(backup))
}

fn restore_destination(path: &Path, backup: Option<&tempfile::TempPath>) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("cannot remove partial {}: {error}", path.display()))?;
    }
    if let Some(backup) = backup {
        fs::rename(backup, path)
            .map_err(|error| format!("cannot restore {}: {error}", path.display()))?;
    }
    Ok(())
}

/// Flush one prepared temp's data to storage before it is promoted.
///
/// Opening the file again rather than keeping the writing handle is
/// deliberate: the callers build their temps through different writers, and
/// `fsync` is defined on the file, not on the handle that wrote it.
fn flush_file(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("cannot flush {}: {error}", path.display()))
}

/// Best-effort flush of a destination directory so the rename that published
/// into it survives a crash. Errors are deliberately discarded: see the
/// module docs.
fn flush_directory(path: &Path) {
    let _ = fs::File::open(path).and_then(|dir| dir.sync_all());
}

/// Publish `artifact_temp` and `evidence_temp` to their destinations as one
/// pair, restoring any prior pair if either promotion fails.
///
/// `fail_after_first_for_test` injects a failure between the two renames.
/// It is the only way to exercise the rollback path without a filesystem
/// that can be made to fail on demand, and every non-test caller passes
/// `false`.
///
/// # Errors
///
/// Returns an operator error for a failed backup, flush, rename, or
/// rollback. A rollback failure is appended to the original error rather
/// than replacing it, so the reason publication was abandoned is never lost.
pub(crate) fn publish_pair(
    artifact_temp: &Path,
    artifact: &Path,
    evidence_temp: &Path,
    evidence: &Path,
    fail_after_first_for_test: bool,
) -> Result<(), String> {
    // Flushed before anything is moved aside: a temp that cannot be flushed
    // must not cost the caller its existing published pair.
    flush_file(artifact_temp)?;
    flush_file(evidence_temp)?;
    let artifact_backup = backup_destination(artifact)?;
    let evidence_backup = match backup_destination(evidence) {
        Ok(backup) => backup,
        Err(error) => {
            restore_destination(artifact, artifact_backup.as_ref())?;
            return Err(error);
        }
    };
    let promote = || -> Result<(), String> {
        fs::rename(artifact_temp, artifact)
            .map_err(|error| format!("cannot publish {}: {error}", artifact.display()))?;
        if fail_after_first_for_test {
            return Err("injected evidence publication failure".into());
        }
        fs::rename(evidence_temp, evidence)
            .map_err(|error| format!("cannot publish {}: {error}", evidence.display()))?;
        Ok(())
    };
    if let Err(error) = promote() {
        let artifact_restore = restore_destination(artifact, artifact_backup.as_ref());
        let evidence_restore = restore_destination(evidence, evidence_backup.as_ref());
        flush_directory(parent_or_current(artifact));
        flush_directory(parent_or_current(evidence));
        let rollback_errors = [artifact_restore.err(), evidence_restore.err()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        return if rollback_errors.is_empty() {
            Err(error)
        } else {
            Err(format!(
                "{error}; rollback also failed: {}",
                rollback_errors.join("; ")
            ))
        };
    }
    flush_directory(parent_or_current(artifact));
    flush_directory(parent_or_current(evidence));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_second_publish_restores_both_previous_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("out.glb");
        let evidence = dir.path().join("out.json");
        let artifact_temp = dir.path().join("new.glb");
        let evidence_temp = dir.path().join("new.json");
        fs::write(&artifact, b"old artifact").unwrap();
        fs::write(&evidence, b"old evidence").unwrap();
        fs::write(&artifact_temp, b"new artifact").unwrap();
        fs::write(&evidence_temp, b"new evidence").unwrap();

        let error =
            publish_pair(&artifact_temp, &artifact, &evidence_temp, &evidence, true).unwrap_err();
        assert!(error.contains("injected evidence publication failure"));
        assert_eq!(fs::read(&artifact).unwrap(), b"old artifact");
        assert_eq!(fs::read(&evidence).unwrap(), b"old evidence");
    }

    #[test]
    fn output_identity_rejects_lexical_aliases() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        let direct = dir.path().join("same.glb");
        let alias = dir.path().join("nested/../same.glb");
        assert_ne!(direct, alias);
        assert_eq!(
            destination_identity(&direct).unwrap(),
            destination_identity(&alias).unwrap()
        );
    }

    #[test]
    fn a_missing_artifact_temp_costs_no_existing_pair() {
        // The flush runs before any destination is moved aside, so a temp
        // that cannot be opened leaves both prior members untouched. Without
        // the ordering the artifact destination is backed up first and the
        // failure surfaces after it has already been moved.
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("out.glb");
        let evidence = dir.path().join("out.json");
        let artifact_temp = dir.path().join("absent.glb");
        let evidence_temp = dir.path().join("new.json");
        fs::write(&artifact, b"old artifact").unwrap();
        fs::write(&evidence, b"old evidence").unwrap();
        fs::write(&evidence_temp, b"new evidence").unwrap();

        let error =
            publish_pair(&artifact_temp, &artifact, &evidence_temp, &evidence, false).unwrap_err();
        assert!(error.contains("cannot flush"), "{error}");
        assert_eq!(fs::read(&artifact).unwrap(), b"old artifact");
        assert_eq!(fs::read(&evidence).unwrap(), b"old evidence");
    }
}
