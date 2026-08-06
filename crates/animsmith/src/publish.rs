//! Atomic artifact/evidence publication shared by every producer command.
//!
//! A producer prepares both members of its pair as temporary files beside
//! their destinations, then hands them here. Publication moves the existing
//! artifact aside, promotes the artifact temp, promotes the evidence temp,
//! and — on any failure — puts the previous artifact back, so a run either
//! publishes a complete new pair or leaves the previous one exactly as it
//! found it.
//!
//! # What a crash between the two renames leaves
//!
//! The renames are individually atomic but not atomic together, so a process
//! killed between them leaves the new artifact beside the *previous*
//! evidence. That is the deliberate choice: only the artifact destination is
//! backed up. Moving the previous evidence aside first would make the same
//! kill leave the new artifact with no evidence at all, and a pair whose
//! members disagree is detectable from the evidence's own record of the
//! artifact digest, where a missing member is not.
//!
//! # Permissions
//!
//! Both members are promoted from [`tempfile`] temporaries, so a published
//! file carries that crate's `0600` rather than the `0644` a `create` under
//! the process umask would produce. This is shared by every producer that
//! publishes this way and is not specific to any one of them.
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

/// A **destination**'s identity for distinctness checks: its canonical parent
/// joined with its own file name.
///
/// Canonicalizing the parent collapses `.`, `..`, and symlinked directories,
/// so two lexically different arguments naming one file compare equal. It
/// deliberately does **not** resolve a symlinked final component, because a
/// destination is reached by [`fs::rename`], which *replaces* the link rather
/// than following it: publishing to `latest.glb -> store/rig.glb` leaves
/// `store/rig.glb` untouched, so the two are genuinely different
/// destinations. The canonical form exists for this comparison only and is
/// never recorded in evidence: evidence keeps the operator's declared path
/// verbatim.
///
/// An **input** must not use this function — see [`input_identity`], whose
/// doc comment carries the argument for the asymmetry.
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

/// An **input**'s identity for distinctness checks: its fully canonical path,
/// final component included.
///
/// An input is reached by [`fs::read`], which *follows* a symlinked final
/// component, so the file an input argument names is the link's target — and
/// only the target's identity can tell a caller whether the input and a
/// destination are the same file. Comparing an input by
/// [`destination_identity`] instead is silently destructive: with
/// `latest.glb -> store/rig.glb`, the pair `--input latest.glb -o
/// store/rig.glb` compares as two different files, publication renames over
/// `store/rig.glb`, and the source asset the run read is gone.
///
/// A **hard link is deliberately not caught by this**, and that is the
/// intended reading rather than a gap. Canonicalization resolves each of an
/// inode's directory entries to its own name, so an input and an output that
/// are two links to one inode compare as distinct — and they are distinct in
/// the sense that matters: the output is reached by [`fs::rename`], which
/// replaces only the output's entry, leaving the input's name bound to the
/// original inode and its bytes intact. Refusing the pair would refuse an
/// invocation whose source survives it. Pinned by
/// `a_hardlinked_input_and_output_publish_with_the_source_surviving`.
///
/// Like [`destination_identity`], the canonical form exists for the
/// distinctness comparison only and is never serialized.
///
/// # Errors
///
/// Returns an operator error when the path cannot be resolved — which
/// includes the input not existing, so the message is phrased as the read
/// failure it is.
pub(crate) fn input_identity(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
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
/// pair, restoring the prior artifact if either promotion fails.
///
/// The evidence destination is never moved aside and never removed: it is
/// written by one rename, so a failure anywhere leaves the previous evidence
/// where it was. See the module docs for why that is stronger than backing it
/// up.
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
    // Only the artifact is moved aside. The evidence destination needs no
    // backup: it is written by exactly one rename, which either succeeds — in
    // which case there is nothing to restore — or leaves the previous
    // evidence in place untouched. Backing it up anyway would put the
    // previous evidence under a temporary name across both renames, so a
    // process killed between them would leave the new artifact with *no*
    // evidence beside it; without the backup that same kill leaves the new
    // artifact with the *old* evidence, a complete pair whose members
    // disagree. A mismatched pair is detectable — the evidence records the
    // artifact's digest — and a missing one is not.
    let artifact_backup = backup_destination(artifact)?;
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
        // The evidence destination is deliberately not touched here: either
        // its rename never ran, or it ran and this failure came from
        // somewhere the rename cannot have reached. Removing it would destroy
        // the previous evidence the missing backup is there to preserve.
        let artifact_restore = restore_destination(artifact, artifact_backup.as_ref());
        flush_directory(parent_or_current(artifact));
        return match artifact_restore {
            Ok(()) => Err(error),
            Err(rollback) => Err(format!("{error}; rollback also failed: {rollback}")),
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
