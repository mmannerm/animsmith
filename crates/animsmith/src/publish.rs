//! Atomic artifact/evidence publication shared by every producer command.
//!
//! A producer prepares both members of its pair as temporary files beside
//! their destinations, then hands them here. Publication moves the existing
//! artifact aside, promotes the artifact temp, promotes the evidence temp,
//! and — on any failure — puts the previous artifact back, so a run either
//! publishes a complete new pair or leaves the previous one exactly as it
//! found it.
//!
//! It also owns the CLI's checked stdout boundary. Human-readable output goes
//! through [`emit_text`], [`emit_text_lines`], or [`emit_text_chunks`], while
//! [`serialize_record`] produces pretty JSON bytes and [`emit`] writes them to
//! stdout. A producer calls it once and hands the same `Vec<u8>` to its
//! evidence file and to stdout, so the two cannot drift apart;
//! [`crate::render::print_json`] routes the output-v12 envelopes through the
//! same pair, so every `--format json` path renders and fails alike.
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

use animsmith_core::sha256_hex;
use serde::Serialize;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use animsmith_core::{
    DependencyClosureCoverageReasonV1, DependencyClosureV1, DependencyReferenceTargetV1,
    DependencyResourceRefusalReasonV1,
};
use animsmith_gltf::fix::{FixReport, Repair};

/// Serialize one record or envelope as the pretty, newline-terminated JSON
/// this CLI writes everywhere.
///
/// A producer calls this once per run and hands the resulting bytes to every
/// destination, so its evidence file and its `--format json` stream being
/// identical is a property of the construction rather than an agreement
/// between two serializers that a test has to keep re-checking. It is also
/// why a producer does not go through [`crate::render::print_json`]: that
/// serializes afresh from the value, which would make the agreement a
/// coincidence again.
///
/// # Errors
///
/// Returns an operator error when a value refuses to serialize — which
/// `scale`'s `Finite` wrapper makes happen for any non-finite number on the
/// evidence path. A record that cannot be rendered truthfully is diagnosed,
/// never panicked over and never silently dropped.
pub(crate) fn serialize_record<T: Serialize>(record: &T) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(record)
        .map_err(|error| format!("cannot serialize JSON output: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Write exactly these bytes to stdout, diagnosing a write failure on stderr
/// rather than raising it.
///
/// Takes bytes rather than a value so a caller cannot accidentally emit a
/// second serialization of the record it already staged, and never panics the
/// way the `println!` this replaced did.
///
/// # Why a failure here does not change the exit code
///
/// By the time this runs the work is over: the pair is already published, or
/// the refusal is already determined and is a fact about the asset either
/// way. The failure is in **reporting** the run, not in performing it.
///
/// Raising it would be actively wrong, not merely noisy. `scale … --format
/// json | head` on a refused asset would exit `2` instead of `1`, turning an
/// asset-property refusal into an operator error — the exact inversion the
/// exit-code split documented on [`crate::scale`] exists to prevent. On the
/// published path it would report `2` for a run that left a complete, correct
/// pair on disk, contradicting what exit `2` means everywhere else in this
/// CLI. So the diagnosis goes to stderr and the outcome stands.
pub(crate) fn emit(bytes: &[u8]) {
    // Locked once for the whole record rather than per `write` call, so a
    // concurrently printed line cannot land inside the JSON document.
    if let Err(error) = emit_to(&mut std::io::stdout().lock(), bytes) {
        // Best effort, and deliberately not `eprint!`: if stderr is gone too
        // then there is nothing left to report with, and `eprint!` would
        // panic for exactly the reason stdout just failed.
        diagnose_write_failure(&error);
    }
}

/// Write one complete, already-rendered standalone JSON result to stdout.
///
/// Unlike [`emit`], this returns delivery failures to its caller. It is only
/// for commands whose sole durable outcome is this one immutable stdout
/// result: when delivery fails, no usable result was published and the CLI
/// must report an operator error. Callers serialize the complete record before
/// calling this function, so no partial serialization can reach stdout.
///
/// Ordinary check, lint, and producer streams intentionally continue to use
/// [`emit`]. Their outcome has already been determined (and producers may
/// already have published their sidecar evidence) before stdout is attempted.
pub(crate) fn emit_required_json(bytes: &[u8]) -> Result<(), String> {
    emit_required_json_to(&mut std::io::stdout().lock(), bytes)
}

/// Write one already-rendered human-readable result to stdout, diagnosing a
/// write failure on stderr without changing the command's outcome.
///
/// Rendering stays in [`crate::render`]; this boundary owns only the fallible
/// I/O. A checked `write_all` replaces `print!`, whose hidden stdout write
/// panics on a closed pipe. The stderr diagnosis is itself best effort because
/// a command may have lost both output streams.
pub(crate) fn emit_text(text: &str) {
    emit_text_with(
        &mut std::io::stdout().lock(),
        &mut std::io::stderr(),
        text.as_bytes(),
    );
}

/// Best-effort checked human-readable stderr delivery.
///
/// Asset refusals use stderr in text mode, and post-parse operator errors use
/// the same boundary after rendering. A closed stderr cannot change the
/// already-established outcome or panic while trying to report it.
pub(crate) fn emit_error_text(text: &str) {
    let _ = std::io::stderr().write_all(text.as_bytes());
}

/// Write rendered human-readable lines to stdout under one lock.
///
/// This is the iterator-shaped counterpart to [`emit_text`]. It appends the
/// newline that `println!("{line}")` supplied, stops after the first failed
/// write, and emits at most one diagnosis for the attempted stream.
pub(crate) fn emit_text_lines(lines: impl IntoIterator<Item = String>) {
    emit_text_lines_with(&mut std::io::stdout().lock(), &mut std::io::stderr(), lines);
}

/// Ask clap to deliver its already-styled help or version output, diagnosing
/// a failed stdout write without changing the successful parser outcome.
///
/// This deliberately uses [`clap::Error::print`] instead of formatting the
/// rendered value into a `String`: `StyledStr`'s `Display` implementation
/// strips ANSI styling, while clap's writer preserves its configured
/// Auto/Always/Never color policy.
pub(crate) fn emit_clap_output(output: &clap::Error) {
    if let Err(error) = output.print() {
        diagnose_write_failure(&format!("cannot write text output to stdout: {error}"));
    }
}

/// Render and write all `fix` reports without exposing a transcript to command
/// dispatch.
///
/// Rendering stays lazy inside this checked boundary: a failed write stops
/// before a later report is pulled or rendered. The whole stream owns one
/// stdout lock and at most one diagnosis.
pub(crate) fn emit_fix_reports<'a>(
    reports: impl IntoIterator<Item = &'a (Repair, FixReport)>,
    target: Option<&'a Path>,
) {
    emit_fix_reports_with(
        &mut std::io::stdout().lock(),
        &mut std::io::stderr(),
        reports,
        target,
    );
}

/// Write exact rendered human-readable chunks to stdout under one lock.
///
/// Unlike [`emit_text_lines`], this does not add separators: callers use it
/// when independently rendered parts already carry their own newlines. The
/// iterator remains lazy, so a failed stream does not retain or render the
/// rest of a potentially asset-sized transcript.
#[cfg_attr(not(feature = "fbx"), allow(dead_code))]
pub(crate) fn emit_text_chunks(chunks: impl IntoIterator<Item = String>) {
    emit_text_chunks_with(
        &mut std::io::stdout().lock(),
        &mut std::io::stderr(),
        chunks,
    );
}

/// Best-effort reporting for a stdout failure. Deliberately ignores stderr's
/// own error so losing both streams can never turn reporting into a panic.
fn diagnose_write_failure(error: &str) {
    diagnose_write_failure_to(&mut std::io::stderr(), error);
}

fn diagnose_write_failure_to(sink: &mut impl std::io::Write, error: &str) {
    let _ = sink.write_all(crate::render::render_operator_error(error).as_bytes());
}

/// Write exactly these bytes to `sink`, reporting a failure as a typed error.
///
/// Split from [`emit`] purely so the failure path is reachable from a unit
/// test: stdout cannot be made to fail on demand from inside the process, and
/// a broken pipe or a full disk turning into a panic rather than a diagnosed
/// error is exactly the behaviour this replaced.
///
/// [`std::io::Write::write_all`] also covers the short-write case: it loops
/// until the buffer is drained and reports [`std::io::ErrorKind::WriteZero`]
/// once the sink stops accepting bytes, so a partially written record is an
/// error here rather than a silently truncated one.
///
/// # Errors
///
/// Returns an operator error naming the underlying I/O failure.
fn emit_to(sink: &mut impl std::io::Write, bytes: &[u8]) -> Result<(), String> {
    sink.write_all(bytes)
        .map_err(|error| format!("cannot write JSON output to stdout: {error}"))
}

fn emit_required_json_to(sink: &mut impl std::io::Write, bytes: &[u8]) -> Result<(), String> {
    emit_to(sink, bytes)?;
    sink.flush()
        .map_err(|error| format!("cannot write JSON output to stdout: {error}"))
}

/// Checked human-readable stdout write, split out so failure paths are unit
/// testable without replacing the process stdout handle.
fn emit_text_to(sink: &mut impl std::io::Write, bytes: &[u8]) -> Result<(), String> {
    sink.write_all(bytes)
        .map_err(|error| format!("cannot write text output to stdout: {error}"))
}

fn report_text_result_to(sink: &mut impl std::io::Write, result: Result<(), String>) {
    if let Err(error) = result {
        diagnose_write_failure_to(sink, &error);
    }
}

fn emit_text_with(
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
    bytes: &[u8],
) {
    report_text_result_to(stderr, emit_text_to(stdout, bytes));
}

fn emit_text_lines_with(
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
    lines: impl IntoIterator<Item = String>,
) {
    report_text_result_to(stderr, emit_text_lines_to(stdout, lines));
}

fn emit_text_lines_to(
    sink: &mut impl std::io::Write,
    lines: impl IntoIterator<Item = String>,
) -> Result<(), String> {
    for line in lines {
        emit_text_to(sink, line.as_bytes())?;
        emit_text_to(sink, b"\n")?;
    }
    Ok(())
}

fn emit_text_line_groups_to<T, G, L>(
    sink: &mut impl std::io::Write,
    groups: G,
    render: impl FnMut(T) -> L,
) -> Result<(), String>
where
    G: IntoIterator<Item = T>,
    L: IntoIterator<Item = String>,
{
    emit_text_lines_to(sink, groups.into_iter().flat_map(render))
}

fn emit_fix_reports_to<'a>(
    sink: &mut impl std::io::Write,
    reports: impl IntoIterator<Item = &'a (Repair, FixReport)>,
    target: Option<&'a Path>,
) -> Result<(), String> {
    emit_text_line_groups_to(sink, reports, |(repair, report)| {
        crate::render::render_fix_report(*repair, report, target)
    })
}

fn emit_fix_reports_with<'a>(
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
    reports: impl IntoIterator<Item = &'a (Repair, FixReport)>,
    target: Option<&'a Path>,
) {
    report_text_result_to(stderr, emit_fix_reports_to(stdout, reports, target));
}

#[cfg_attr(not(feature = "fbx"), allow(dead_code))]
fn emit_text_chunks_to(
    sink: &mut impl std::io::Write,
    chunks: impl IntoIterator<Item = String>,
) -> Result<(), String> {
    for chunk in chunks {
        emit_text_to(sink, chunk.as_bytes())?;
    }
    Ok(())
}

#[cfg_attr(not(feature = "fbx"), allow(dead_code))]
fn emit_text_chunks_with(
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
    chunks: impl IntoIterator<Item = String>,
) {
    report_text_result_to(stderr, emit_text_chunks_to(stdout, chunks));
}

/// The directory a path lives in, treating a bare file name as `.`.
pub(crate) fn parent_or_current(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

/// A **destination**'s identity for distinctness checks.
///
/// Canonicalizing the parent collapses `.`, `..`, and symlinked directories,
/// so two lexically different arguments naming one file compare equal. An
/// existing non-symlink final component is canonicalized as well, which lets
/// the filesystem apply its own case semantics. A final symlink is deliberately
/// kept as its canonical parent plus declared file name because a destination
/// is reached by [`fs::rename`], which *replaces* the link rather than following
/// it: publishing to `latest.glb -> store/rig.glb` leaves `store/rig.glb`
/// untouched, so the two are genuinely different destinations. A missing
/// final component likewise uses the canonical parent plus its declared name.
/// The canonical form exists for this comparison only and is never recorded in
/// evidence: evidence keeps the operator's declared path verbatim.
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
    destination_identity_below(path, parent)
}

fn destination_identity_below(path: &Path, parent: PathBuf) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("output {} has no file name", path.display()))?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_symlink() => fs::canonicalize(path)
            .map_err(|error| format!("cannot resolve existing output {}: {error}", path.display())),
        Ok(_) => Ok(parent.join(file_name)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(parent.join(file_name)),
        Err(error) => Err(format!(
            "cannot inspect existing output {}: {error}",
            path.display()
        )),
    }
}

/// Resolve a retained dependency's destination-style entry identity when its
/// parent exists.
///
/// A missing intermediate directory cannot alias a publication destination:
/// both producers have already required each destination's parent to exist.
/// Returning `None` preserves that honest unavailable dependency without
/// turning every unrelated invocation into an operator error. When only the
/// final entry is missing, the existing parent plus declared name still lets
/// the ordinary comparison reject publication to that exact key.
fn retained_dependency_identity(path: &Path) -> Result<Option<PathBuf>, String> {
    let parent = parent_or_current(path);
    let parent = match fs::canonicalize(parent) {
        Ok(parent) => parent,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "cannot resolve external dependency directory {}: {error}",
                parent.display()
            ));
        }
    };
    destination_identity_below(path, parent).map(Some)
}

pub(crate) struct PublicationDestination<'a> {
    label: &'a str,
    identity: PathBuf,
    entry_name_probe: Option<tempfile::TempDir>,
}

impl<'a> PublicationDestination<'a> {
    pub(crate) fn new(label: &'a str, path: &Path) -> Result<Self, String> {
        let identity = destination_identity(path)?;
        let needs_entry_name_probe = match fs::symlink_metadata(&identity) {
            Ok(metadata) => metadata.file_type().is_symlink(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => {
                return Err(format!(
                    "cannot inspect existing {label} {}: {error}",
                    path.display()
                ));
            }
        };
        let entry_name_probe = if needs_entry_name_probe {
            let parent = identity
                .parent()
                .ok_or_else(|| format!("{label} {} has no parent directory", path.display()))?;
            let file_name = identity
                .file_name()
                .ok_or_else(|| format!("{label} {} has no file name", path.display()))?;
            let probe = tempfile::Builder::new()
                .prefix(".animsmith-name-identity-")
                .tempdir_in(parent)
                .map_err(|error| {
                    format!(
                        "cannot inspect filesystem name semantics for {label} {}: {error}",
                        path.display()
                    )
                })?;
            fs::File::create(probe.path().join(file_name)).map_err(|error| {
                format!(
                    "cannot inspect filesystem name semantics for {label} {}: {error}",
                    path.display()
                )
            })?;
            Some(probe)
        } else {
            None
        };
        Ok(Self {
            label,
            identity,
            entry_name_probe,
        })
    }

    pub(crate) fn identity(&self) -> &Path {
        &self.identity
    }

    pub(crate) fn aliases_destination(&self, other: &Self) -> Result<bool, String> {
        if self.identity == other.identity {
            return Ok(true);
        }
        if self.identity.parent() != other.identity.parent() {
            return Ok(false);
        }
        if let Some(probe) = &self.entry_name_probe {
            return probe_contains_name(probe, &other.identity, self.label, other.label);
        }
        if let Some(probe) = &other.entry_name_probe {
            return probe_contains_name(probe, &self.identity, other.label, self.label);
        }
        Ok(false)
    }

    fn aliases(&self, dependency: &Path) -> Result<bool, String> {
        if self.identity == dependency {
            return Ok(true);
        }
        let Some(probe) = &self.entry_name_probe else {
            return Ok(false);
        };
        if self.identity.parent() != dependency.parent() {
            return Ok(false);
        }
        let dependency_name = dependency.file_name().ok_or_else(|| {
            format!(
                "external dependency {} has no file name",
                dependency.display()
            )
        })?;
        match fs::symlink_metadata(probe.path().join(dependency_name)) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(format!(
                "cannot compare external dependency {} with {} using filesystem name semantics: {error}",
                dependency.display(),
                self.label
            )),
        }
    }
}

fn probe_contains_name(
    probe: &tempfile::TempDir,
    candidate: &Path,
    probe_label: &str,
    candidate_label: &str,
) -> Result<bool, String> {
    let candidate_name = candidate
        .file_name()
        .ok_or_else(|| format!("{candidate_label} {} has no file name", candidate.display()))?;
    match fs::symlink_metadata(probe.path().join(candidate_name)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "cannot compare {probe_label} with {candidate_label} using filesystem name semantics: {error}"
        )),
    }
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

/// Reject publication destinations that name a retained external dependency.
///
/// Rooted format loaders may consume more files than their primary input. A
/// producer must treat every safe external key retained by the closure as
/// source data for the same destructive-alias check it already applies to the
/// primary input. This includes a keyed sidecar whose capture was unavailable
/// or refused; otherwise a successful pair publication can replace that linked
/// texture or other sidecar with the artifact or evidence record.
///
/// # Errors
///
/// Returns an operator error when the closure stopped at its resource budget,
/// a dependency path cannot be inspected, or a key names one of the supplied
/// destinations.
pub(crate) fn require_external_dependencies_safe_for_publication(
    command: &str,
    resource_root: &Path,
    closure: &DependencyClosureV1,
    destinations: &[(&str, &Path)],
) -> Result<(), String> {
    let destinations = destinations
        .iter()
        .map(|(label, path)| PublicationDestination::new(label, path))
        .collect::<Result<Vec<_>, String>>()?;
    if closure
        .coverage()
        .reasons()
        .contains(&DependencyClosureCoverageReasonV1::ResourceBudgetExceeded)
    {
        return Err(format!(
            "{command} dependency closure exceeded its resource budget, so publication cannot prove every source sidecar distinct"
        ));
    }
    for reference in closure.references() {
        let key = match reference.target() {
            DependencyReferenceTargetV1::External { key }
            | DependencyReferenceTargetV1::Refused { key: Some(key), .. }
            | DependencyReferenceTargetV1::Unavailable { key: Some(key), .. } => key,
            DependencyReferenceTargetV1::Primary
            | DependencyReferenceTargetV1::Refused { key: None, .. }
            | DependencyReferenceTargetV1::Unavailable { key: None, .. } => continue,
        };
        if matches!(
            reference.target(),
            DependencyReferenceTargetV1::Refused {
                reason: DependencyResourceRefusalReasonV1::Symlink,
                ..
            }
        ) {
            return Err(format!(
                "{command} external dependency {:?} is a symlink and cannot be published safely",
                key.as_str()
            ));
        }
        let Some(dependency) = retained_dependency_identity(&resource_root.join(key.as_str()))?
        else {
            continue;
        };
        for destination in &destinations {
            if destination.aliases(&dependency)? {
                return Err(format!(
                    "{command} external dependency {:?} and {} must be different paths, but both resolve to {}",
                    key.as_str(),
                    destination.label,
                    dependency.display()
                ));
            }
        }
    }
    Ok(())
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
    // Opened for writing, not merely for reading: Windows backs `sync_all` with
    // `FlushFileBuffers`, which requires write access and fails with "Access is
    // denied" on a read-only handle. A read-only open works on Unix and would
    // make every publication on Windows fail at its first step.
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
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

/// Publish one already-staged sidecar with the same durable single-file path
/// used by producer pairs: fsync the temporary file, rename it into place,
/// then best-effort fsync its directory.
///
/// The caller creates the temporary beside `destination`; therefore rename is
/// atomic and a pre-publication refusal has never touched an existing output.
pub(crate) fn publish_single(temp: &Path, destination: &Path) -> Result<(), String> {
    flush_file(temp)?;
    fs::rename(temp, destination)
        .map_err(|error| format!("cannot publish {}: {error}", destination.display()))?;
    flush_directory(parent_or_current(destination));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sink that refuses every write, the way a closed pipe does.
    struct BrokenPipe;

    impl std::io::Write for BrokenPipe {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "the reader is gone",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A buffered destination that accepts bytes but fails only when asked to
    /// commit them, matching the stdout buffering boundary.
    struct FlushFailure {
        accepted: Vec<u8>,
    }

    impl std::io::Write for FlushFailure {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.accepted.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "destination rejected the flush",
            ))
        }
    }

    /// A sink that takes a prefix and then stops accepting bytes, which is
    /// how a filesystem that has just filled up presents itself to `write`.
    struct ShortWriter {
        accepted: Vec<u8>,
        budget: usize,
    }

    impl std::io::Write for ShortWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let taken = buf.len().min(self.budget);
            self.accepted.extend_from_slice(&buf[..taken]);
            self.budget -= taken;
            Ok(taken)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_sink_that_refuses_the_record_is_a_typed_error_and_not_a_panic() {
        // The behaviour this replaced was `println!`'s panic, which reaches
        // the operator as exit 101 and a backtrace rather than a diagnosis.
        // What `emit` then does with the error — diagnose it and leave the
        // outcome's exit code alone — is pinned end to end by the CLI tests.
        let error =
            emit_to(&mut BrokenPipe, b"{}\n").expect_err("a closed pipe must not be ignored");
        assert!(
            error.starts_with("cannot write JSON output to stdout"),
            "{error}"
        );
        assert!(error.contains("the reader is gone"), "{error}");
    }

    #[test]
    fn standalone_result_delivery_is_a_typed_operator_error() {
        let error = emit_required_json_to(&mut BrokenPipe, b"{}\n")
            .expect_err("a standalone result must report a failed delivery");
        assert!(
            error.starts_with("cannot write JSON output to stdout"),
            "{error}"
        );

        let mut buffered = FlushFailure {
            accepted: Vec::new(),
        };
        let error = emit_required_json_to(&mut buffered, b"{}\n")
            .expect_err("a buffered standalone result must flush before success");
        assert_eq!(buffered.accepted, b"{}\n");
        assert!(error.contains("destination rejected the flush"), "{error}");
    }

    #[test]
    fn checked_text_writes_cover_single_results_and_iterator_lines() {
        let single = emit_text_to(&mut BrokenPipe, b"summary\n")
            .expect_err("a closed text stream must be diagnosed");
        assert!(
            single.starts_with("cannot write text output to stdout"),
            "{single}"
        );

        let lines = emit_text_lines_to(&mut BrokenPipe, ["first".to_owned(), "second".to_owned()])
            .expect_err("a closed line stream must be diagnosed");
        assert!(
            lines.starts_with("cannot write text output to stdout"),
            "{lines}"
        );

        let chunks = emit_text_chunks_to(
            &mut BrokenPipe,
            ["first\n".to_owned(), "second\n".to_owned()],
        )
        .expect_err("a closed chunk stream must be diagnosed");
        assert!(
            chunks.starts_with("cannot write text output to stdout"),
            "{chunks}"
        );
    }

    #[test]
    fn failed_text_iterators_do_not_render_unreached_transcript_parts() {
        let mut rendered = 0;
        let chunks = ["first", "unreached"].into_iter().map(|chunk| {
            rendered += 1;
            assert_eq!(rendered, 1, "closed output must stop lazy rendering");
            format!("{chunk}\n")
        });
        emit_text_chunks_to(&mut BrokenPipe, chunks).expect_err("first chunk must fail");
        assert_eq!(rendered, 1);
    }

    #[test]
    fn line_and_chunk_streams_stop_after_a_write_zero_without_pulling_the_tail() {
        let mut line_pulls = 0;
        let lines = ["first", "failing", "unreached"].into_iter().map(|line| {
            line_pulls += 1;
            line.to_owned()
        });
        let mut line_stdout = ShortWriter {
            accepted: Vec::new(),
            budget: b"first\n".len(),
        };
        let mut line_stderr = Vec::new();
        emit_text_lines_with(&mut line_stdout, &mut line_stderr, lines);
        assert_eq!(line_stdout.accepted, b"first\n");
        assert_eq!(line_pulls, 2, "the item after the failed write stays lazy");
        let line_diagnostic = String::from_utf8(line_stderr).unwrap();
        assert_eq!(
            line_diagnostic
                .matches("animsmith: cannot write text output to stdout")
                .count(),
            1,
            "{line_diagnostic}"
        );
        assert!(
            line_diagnostic.contains("failed to write whole buffer"),
            "the accepted prefix ends in WriteZero, not BrokenPipe: {line_diagnostic}"
        );

        let mut chunk_pulls = 0;
        let chunks = ["summary\n", "optional\n", "unreached\n"]
            .into_iter()
            .map(|chunk| {
                chunk_pulls += 1;
                chunk.to_owned()
            });
        let mut chunk_stdout = ShortWriter {
            accepted: Vec::new(),
            budget: b"summary\n".len(),
        };
        let mut chunk_stderr = Vec::new();
        emit_text_chunks_with(&mut chunk_stdout, &mut chunk_stderr, chunks);
        assert_eq!(chunk_stdout.accepted, b"summary\n");
        assert_eq!(chunk_pulls, 2, "the item after the failed write stays lazy");
        let chunk_diagnostic = String::from_utf8(chunk_stderr).unwrap();
        assert_eq!(
            chunk_diagnostic
                .matches("animsmith: cannot write text output to stdout")
                .count(),
            1,
            "{chunk_diagnostic}"
        );
        assert!(
            chunk_diagnostic.contains("failed to write whole buffer"),
            "the accepted prefix ends in WriteZero, not BrokenPipe: {chunk_diagnostic}"
        );
    }

    #[test]
    fn conversion_chunks_preserve_order_and_stop_after_the_first_optional_failure() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let summary = animsmith_gltf::write::write(
            &animsmith_core::model::Document::default(),
            &dir.path().join("summary.glb"),
        )
        .expect("obtains a real writer summary");
        let first_summary =
            crate::render::render_write_summary(Path::new("converted.glb"), &summary);
        let transcript = || {
            [
                first_summary.clone(),
                "baked 1 static mesh instance(s) into identity-root geometry\n".to_owned(),
                "applied material texture recipe; emitted 2 texture(s)\n".to_owned(),
            ]
            .into_iter()
        };

        let mut writable_stdout = Vec::new();
        let mut writable_stderr = Vec::new();
        emit_text_chunks_with(&mut writable_stdout, &mut writable_stderr, transcript());
        assert_eq!(
            String::from_utf8(writable_stdout).unwrap(),
            transcript().collect::<Vec<_>>().concat(),
            "the write summary precedes bake and recipe summaries"
        );
        assert!(writable_stderr.is_empty());

        let first = transcript().next().unwrap();
        let mut pulls = 0;
        let chunks = transcript().inspect(|_| pulls += 1);
        let mut stdout = ShortWriter {
            accepted: Vec::new(),
            budget: first.len(),
        };
        let mut stderr = Vec::new();
        emit_text_chunks_with(&mut stdout, &mut stderr, chunks);
        assert_eq!(stdout.accepted, first.as_bytes());
        assert_eq!(pulls, 2, "the recipe chunk after the failure is not pulled");
        let diagnostic = String::from_utf8(stderr).unwrap();
        assert_eq!(
            diagnostic
                .matches("animsmith: cannot write text output to stdout")
                .count(),
            1,
            "{diagnostic}"
        );
    }

    #[test]
    fn failed_fix_streams_do_not_pull_or_render_later_reports() {
        let mut pulled = 0;
        let reports = [
            (Repair::QuatNorm, FixReport::default()),
            (Repair::QuatFlip, FixReport::default()),
        ];
        let reports = reports.iter().inspect(|_| {
            pulled += 1;
            assert_eq!(pulled, 1, "closed output must not pull a later report");
        });
        let mut stderr = Vec::new();
        emit_fix_reports_with(&mut BrokenPipe, &mut stderr, reports, None);
        assert_eq!(pulled, 1);
        let diagnostic = String::from_utf8(stderr).unwrap();
        assert_eq!(
            diagnostic
                .matches("animsmith: cannot write text output to stdout")
                .count(),
            1,
            "{diagnostic}"
        );
    }

    #[test]
    fn failed_fix_stream_stops_inside_a_real_multi_track_report() {
        let mut document = animsmith_testkit::two_bone_rotation_doc(
            "sway",
            animsmith_testkit::quats_from_angles(&[0.0, 0.4, 0.8, 1.2, 1.6]),
            false,
        );
        let mut second = document.clips[0].tracks[0].clone();
        second.bone = 0;
        document.clips[0].tracks.push(second);
        for track in &mut document.clips[0].tracks {
            let animsmith_core::model::TrackValues::Quats(values) = &mut track.values else {
                panic!("rotation values");
            };
            values[1] = -values[1];
            values[3] = -values[3];
        }

        let dir = tempfile::tempdir().expect("temporary directory");
        let input = dir.path().join("multi-track.glb");
        animsmith_gltf::write::write(&document, &input).expect("writes repair fixture");
        let mut session = animsmith_gltf::fix::FixSession::read(&input).expect("reads fixture");
        let report = session.apply(Repair::QuatFlip);
        assert_eq!(report.tracks.len(), 2, "fixture has two repaired tracks");
        assert_eq!(
            crate::render::render_fix_report(Repair::QuatFlip, &report, None)
                .take(2)
                .count(),
            2,
            "the first report really has multiple track-derived lines"
        );

        let reports = [(Repair::QuatFlip, report)];
        let mut stderr = Vec::new();
        emit_fix_reports_with(&mut BrokenPipe, &mut stderr, reports.iter(), None);
        let diagnostic = String::from_utf8(stderr).unwrap();
        assert_eq!(
            diagnostic
                .matches("animsmith: cannot write text output to stdout")
                .count(),
            1,
            "the first failed track line diagnoses the whole lazy stream once: {diagnostic}"
        );

        let rendered_lines = std::cell::Cell::new(0);
        let error = emit_text_line_groups_to(&mut BrokenPipe, [()], |_| {
            ["first track", "second track", "summary"]
                .into_iter()
                .map(|line| {
                    rendered_lines.set(rendered_lines.get() + 1);
                    line.to_owned()
                })
        })
        .expect_err("the first track-derived line fails");
        assert!(
            error.starts_with("cannot write text output to stdout"),
            "{error}"
        );
        assert_eq!(
            rendered_lines.get(),
            1,
            "a first-line failure must not render later lines in the same report"
        );
    }

    #[test]
    fn a_closed_stderr_cannot_turn_a_stdout_diagnosis_into_a_panic() {
        diagnose_write_failure_to(
            &mut BrokenPipe,
            "cannot write text output to stdout: the reader is gone",
        );
    }

    #[test]
    fn command_stdout_sites_cannot_bypass_the_checked_text_boundary() {
        let main_source = include_str!("main.rs");
        for (name, source) in [
            ("main.rs", main_source),
            ("assembly.rs", include_str!("assembly.rs")),
            ("scale.rs", include_str!("scale.rs")),
        ] {
            let bypasses = source
                .lines()
                .enumerate()
                .filter(|(_, line)| {
                    (line.contains("print!(") || line.contains("println!("))
                        && !line.contains("eprint!(")
                        && !line.contains("eprintln!(")
                })
                .map(|(index, line)| format!("{}: {}", index + 1, line.trim()))
                .collect::<Vec<_>>();
            assert!(
                bypasses.is_empty(),
                "{name} has unchecked stdout macros; route them through emit_text or \
                 emit_text_lines:\n{}",
                bypasses.join("\n")
            );
            assert!(
                !source.contains("std::io::stdout")
                    && !source.contains("io::stdout")
                    && !source.contains("stdout().lock")
                    && !source.contains("stdout().write")
                    && !source.contains("stdout().flush"),
                "{name} acquires or writes stdout outside publish.rs"
            );
        }
        assert!(
            !main_source.contains("Cli::parse();"),
            "clap display-help/version bypasses checked stdout through Cli::parse"
        );
        assert!(
            main_source.contains("Cli::try_parse()"),
            "CLI parsing must retain display-help/version for checked delivery"
        );
        assert!(
            main_source.contains("publish::emit_clap_output(&error)"),
            "clap display-help/version must preserve its styled checked writer"
        );
        assert!(
            !main_source.contains("error.render().to_string()"),
            "formatting clap StyledStr through Display strips forced ANSI styling"
        );
        let fix_dispatch = main_source
            .rsplit_once("Cmd::Fix {")
            .and_then(|(_, suffix)| suffix.split_once("#[cfg(feature = \"fbx\")]"))
            .map(|(fix, _)| fix)
            .expect("locates fix dispatch arm");
        assert!(
            fix_dispatch.contains("publish::emit_fix_reports("),
            "fix dispatch must hand reports directly to the specialized checked boundary"
        );
        assert!(
            !fix_dispatch.contains("render_fix_report"),
            "fix dispatch must not obtain a render iterator it could materialize"
        );
        let publish_source = include_str!("publish.rs");
        let diagnosis_wrapper = publish_source
            .split_once("fn diagnose_write_failure(error:")
            .and_then(|(_, suffix)| suffix.split_once("fn diagnose_write_failure_to"))
            .map(|(body, _)| body)
            .expect("locates the production stderr diagnosis wrapper");
        assert!(
            diagnosis_wrapper.contains("diagnose_write_failure_to(&mut std::io::stderr(), error)"),
            "the production diagnosis wrapper must use the checked stderr writer"
        );
        assert!(
            !diagnosis_wrapper.contains("eprint!(")
                && !diagnosis_wrapper.contains("eprintln!(")
                && !diagnosis_wrapper.contains("unwrap("),
            "the production diagnosis wrapper must not panic when stderr is closed"
        );
        let fix_wrapper = publish_source
            .split_once("pub(crate) fn emit_fix_reports")
            .and_then(|(_, suffix)| suffix.split_once("/// Write exact rendered"))
            .map(|(body, _)| body)
            .expect("locates specialized fix emitter");
        for forbidden in ["collect", "Vec<", "Vec::", "from_iter", "render_fix_report"] {
            assert!(
                !fix_wrapper.contains(forbidden),
                "specialized fix wrapper must pass the lazy report iterator through; found {forbidden}"
            );
        }
        let fix_pipeline = publish_source
            .split_once("fn emit_text_line_groups_to")
            .and_then(|(_, suffix)| suffix.split_once("fn emit_fix_reports_with"))
            .map(|(body, _)| body)
            .expect("locates the lazy fix rendering pipeline");
        assert!(
            fix_pipeline.contains("flat_map(render)") && fix_pipeline.contains("render_fix_report"),
            "the specialized fix emitter must stream rendered lines lazily"
        );
        for forbidden in ["collect", "Vec<", "Vec::", "from_iter"] {
            assert!(
                !fix_pipeline.contains(forbidden),
                "the specialized fix pipeline must not retain its transcript; found {forbidden}"
            );
        }
        for (emitter, start, end, route) in [
            (
                "emit",
                "pub(crate) fn emit(bytes:",
                "/// Write one already-rendered",
                "diagnose_write_failure(",
            ),
            (
                "emit_text",
                "pub(crate) fn emit_text(text:",
                "/// Write rendered human-readable lines",
                "emit_text_with(",
            ),
            (
                "emit_text_lines",
                "pub(crate) fn emit_text_lines(",
                "/// Ask clap",
                "emit_text_lines_with(",
            ),
            (
                "emit_clap_output",
                "pub(crate) fn emit_clap_output(",
                "/// Render and write all `fix`",
                "diagnose_write_failure(",
            ),
            (
                "emit_fix_reports",
                "pub(crate) fn emit_fix_reports",
                "/// Write exact rendered",
                "emit_fix_reports_with(",
            ),
            (
                "emit_text_chunks",
                "pub(crate) fn emit_text_chunks(",
                "/// Best-effort reporting",
                "emit_text_chunks_with(",
            ),
        ] {
            let body = publish_source
                .split_once(start)
                .and_then(|(_, suffix)| suffix.split_once(end))
                .map(|(body, _)| body)
                .unwrap_or_else(|| panic!("locates {emitter} implementation"));
            assert!(
                body.contains(route),
                "{emitter} must retain the shared checked diagnostic route {route}"
            );
            assert!(
                !body.contains("eprint!(") && !body.contains("eprintln!("),
                "{emitter} must not bypass the checked stderr diagnosis"
            );
        }
        assert!(
            include_str!("scale.rs").contains("emit_text(&render::render_scale_published("),
            "scale text publication must use the checked text emitter"
        );
        assert!(
            include_str!("assembly.rs").contains("emit_text(&render::render_assemble_published("),
            "assembly text publication must use the checked text emitter"
        );
    }

    #[test]
    fn a_sink_that_stops_mid_record_refuses_rather_than_reporting_success() {
        // `write_all` loops until the buffer drains, so the failure surfaces
        // only once the sink stops taking bytes. A half-written record must
        // not read as a written one.
        let mut sink = ShortWriter {
            accepted: Vec::new(),
            budget: 5,
        };
        let error = emit_to(&mut sink, b"{\"schema\":1}\n")
            .expect_err("a truncated record must not be reported as written");
        assert!(
            error.starts_with("cannot write JSON output to stdout"),
            "{error}"
        );
        assert_eq!(
            sink.accepted, b"{\"sch",
            "only the accepted prefix got through"
        );
    }

    #[test]
    fn a_writable_sink_receives_exactly_the_serialized_record() {
        // Pins both halves of the shared helper: what `serialize_record`
        // produces (pretty, newline-terminated) and that `emit_to` passes it
        // through unchanged rather than re-rendering it.
        let record =
            serialize_record(&serde_json::json!({"schema": 1})).expect("record serializes");
        assert_eq!(record, b"{\n  \"schema\": 1\n}\n");
        let mut sink = Vec::new();
        emit_to(&mut sink, &record).expect("a writable sink takes the record");
        assert_eq!(sink, record);
    }

    /// Every `.rs` file anywhere under this crate's `src/`, as
    /// `(path relative to src/, text)`, sorted.
    ///
    /// Recursive on purpose. The crate is flat today, but a scan that only
    /// reads the top level would keep passing after a module moved into a
    /// subdirectory — a guard that quietly stops guarding, which reads as
    /// coverage while providing none.
    fn crate_sources() -> Vec<(String, String)> {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut sources = Vec::new();
        let mut pending = vec![src.clone()];
        while let Some(directory) = pending.pop() {
            let entries = fs::read_dir(&directory)
                .unwrap_or_else(|error| panic!("reads {}: {error}", directory.display()));
            for entry in entries {
                let path = entry.expect("a source directory entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    let name = path
                        .strip_prefix(&src)
                        .expect("a path found under src/")
                        .to_str()
                        .expect("a UTF-8 source path")
                        .replace('\\', "/");
                    let text = fs::read_to_string(&path)
                        .unwrap_or_else(|error| panic!("reads {}: {error}", path.display()));
                    sources.push((name, text));
                }
            }
        }
        assert!(
            sources.iter().any(|(name, _)| name == "publish.rs"),
            "the scan must reach this crate's own sources"
        );
        sources.sort();
        sources
    }

    /// "Each record is serialized exactly once" is a property of the code's
    /// shape, and no behavioural test can observe it: a second serializer
    /// over one record produces identical bytes, so the byte-identity tests
    /// in `character_assembly_cli.rs` and `scale_cli.rs` pass either way.
    ///
    /// What *is* mechanically checkable is that only one serializer exists.
    /// [`crate::render::print_json`] routes the output-v12 envelopes through
    /// this same helper, so every byte of pretty JSON this CLI writes — an
    /// evidence record or an envelope — is produced here. This is a source
    /// scan rather than a type-level constraint because `serde_json`'s free
    /// functions cannot be made unreachable from inside the crate that
    /// depends on it.
    ///
    /// # What this does and does not see
    ///
    /// It counts **occurrences**, not matching lines, over every `.rs` file
    /// under `src/` recursively. A call `rustfmt` wraps is still caught: Rust
    /// never splits an identifier across lines, so a break at `::` or at the
    /// opening paren leaves the name contiguous on a line of its own —
    /// verified by probe rather than assumed.
    ///
    /// It cannot see an import that renames one of these functions on the way
    /// in (`use serde_json::… as pretty;`). That is a deliberate evasion
    /// rather than something a refactor or a formatter produces, and catching
    /// it would mean parsing rather than scanning.
    #[test]
    fn pretty_json_is_produced_at_exactly_one_site() {
        // Split so this test's own needles do not match themselves when it
        // scans the file it lives in.
        let needles = [
            concat!("to_vec", "_pretty"),
            concat!("to_string", "_pretty"),
            concat!("to_writer", "_pretty"),
        ];
        let sites = crate_sources()
            .into_iter()
            .filter_map(|(name, text)| {
                let hits: usize = needles
                    .iter()
                    .map(|needle| text.matches(needle).count())
                    .sum();
                (hits > 0).then_some((name, hits))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            sites,
            std::collections::BTreeMap::from([("publish.rs".to_owned(), 1)]),
            "pretty JSON has exactly one producer: `publish::serialize_record`. Evidence \
             records go through it once and its bytes reach both the evidence file and \
             stdout; the output-v12 envelopes reach it through `render::print_json`. Route \
             a new call through one of those two entry points instead of adding a second \
             serializer."
        );
    }

    /// A producer's `--format json` stream must be the bytes it published,
    /// not a second rendering of the record.
    ///
    /// [`crate::render::print_json`] shares this module's serializer but still
    /// serializes *afresh from the value*, so a producer calling it would
    /// serialize its record a second time — turning byte identity back into a
    /// coincidence, and one the byte-identity tests cannot see because the
    /// second rendering agrees.
    #[test]
    fn no_producer_module_reaches_for_the_re_serializing_printer() {
        // Split for the same reason as above: publish.rs names the printer in
        // its own documentation.
        let printer = concat!("print", "_json");
        let sources = crate_sources();
        for producer in ["assembly.rs", "scale.rs", "contact_producer.rs"] {
            let (_, text) = sources
                .iter()
                .find(|(name, _)| name == producer)
                .unwrap_or_else(|| panic!("{producer} is part of this crate"));
            assert!(
                !text.contains(printer),
                "{producer} must emit the bytes it already published through `publish::emit` \
                 rather than re-render its record through `render::print_json`"
            );
        }
    }

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
    fn single_publication_replaces_only_after_a_complete_temp_is_staged() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("fragment.json");
        let temp = dir.path().join("fragment.tmp");
        fs::write(&destination, b"old fragment").unwrap();
        fs::write(&temp, b"new fragment").unwrap();

        publish_single(&temp, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new fragment");
        assert!(!temp.exists(), "rename consumed the staged temporary file");
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
