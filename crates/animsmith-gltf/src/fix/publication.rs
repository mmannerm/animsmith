//! Admission for the direct, followed writes performed by `FixSession`.
//!
//! The CLI's staged publisher replaces directory entries with `rename`;
//! repair instead follows final symlinks and changes every hardlink's bytes.
//! Keep these operation-specific identities separate.

use crate::{FixError, LoadError, WriteError, safe_external_buffer_path};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Primary,
    Buffer(usize),
    Image(usize),
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Primary => f.write_str("primary"),
            Self::Buffer(index) => write!(f, "buffer {index}"),
            Self::Image(index) => write!(f, "image {index}"),
        }
    }
}

struct Source {
    role: Role,
    path: PathBuf,
    // Preserve identity when a session's original name is removed before
    // writing. Defer identity errors until write: inspection creates no files
    // and does not require filesystem identity beyond reading the buffers.
    captured: io::Result<Option<FileId>>,
}

pub(super) struct Origin {
    sources: Vec<Source>,
    #[cfg(unix)]
    primary_mode: io::Result<u32>,
}

#[derive(Default)]
pub(super) struct Destinations {
    files: Vec<Destination>,
}

struct Destination {
    role: Role,
    path: PathBuf,
    file: fs::File,
    id: FileId,
    reserved: bool,
}

impl Destination {
    fn cleanup(&mut self) -> io::Result<()> {
        if self.reserved {
            remove_reservation(&self.file, &self.path)?;
            self.reserved = false;
        }
        Ok(())
    }

    fn verify_identity(&self) -> Result<(), WriteError> {
        if file_id(&self.path).map_err(|e| inspect_error(&self.path, &e))? != self.id {
            return Err(WriteError::RepairPublication(format!(
                "destination {} changed after preflight",
                self.path.display()
            )));
        }
        Ok(())
    }

    fn write(&self, bytes: &[u8]) -> Result<(), FixError> {
        // A caller may rename a file between admission and publication. Never
        // reopen and truncate an unrelated replacement: write the owned file.
        self.verify_identity()?;
        let mut file = &self.file;
        let mut write = || -> io::Result<()> {
            file.seek(SeekFrom::Start(0))?;
            // Write before shortening so a refused write need not truncate an
            // unchanged in-place JSON snapshot.
            file.write_all(bytes)?;
            file.set_len(bytes.len() as u64)
        };
        write().map_err(|source| {
            WriteError::Io {
                path: self.path.display().to_string(),
                source,
            }
            .into()
        })
    }
}

impl Drop for Destination {
    fn drop(&mut self) {
        // Normal refusal handles cleanup errors explicitly. This is the
        // fallback for unwinding; interrupted processes may leave empty names.
        let _ = self.cleanup();
    }
}

impl Destinations {
    fn begin_publication(&mut self) -> Result<(), WriteError> {
        for file in &self.files {
            file.verify_identity()?;
        }
        // Admission is complete. Subsequent I/O errors are not a transaction:
        // preserve any created outputs, including a partially written file.
        for file in &mut self.files {
            file.reserved = false;
        }
        Ok(())
    }

    pub(super) fn write_primary(&mut self, bytes: &[u8]) -> Result<(), FixError> {
        self.begin_publication()?;
        self.files
            .iter()
            .find(|file| file.role == Role::Primary)
            .expect("preflight admits the primary")
            .write(bytes)
    }

    pub(super) fn write_buffers(&self, buffers: &[Vec<u8>]) -> Result<(), FixError> {
        for file in &self.files {
            if let Role::Buffer(index) = file.role {
                let bytes = buffers
                    .get(index)
                    .ok_or_else(|| LoadError::Buffer(format!("missing patched buffer {index}")))?;
                file.write(bytes)?;
            }
        }
        Ok(())
    }

    pub(super) fn refuse(mut self, error: FixError) -> FixError {
        let mut failures = Vec::new();
        for file in &mut self.files {
            if let Err(cleanup) = file.cleanup() {
                failures.push(format!("{}: {cleanup}", file.path.display()));
            }
        }
        if failures.is_empty() {
            error
        } else {
            WriteError::RepairPublication(format!(
                "{error}; could not remove new reservations: {}",
                failures.join(", ")
            ))
            .into()
        }
    }
}

#[derive(Clone)]
struct Entry {
    role: Role,
    path: PathBuf,
    output: bool,
}

enum Identity {
    Existing(FileId),
    Missing(PathBuf),
}

impl Origin {
    pub(super) fn capture(input: &Path, gltf: &gltf::Gltf) -> Result<Self, LoadError> {
        let input = std::path::absolute(input).map_err(|source| LoadError::Io {
            path: input.display().to_string(),
            source,
        })?;
        #[cfg(unix)]
        let primary_mode = {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(&input).map(|metadata| metadata.permissions().mode() & 0o7777)
        };
        let root = parent(&input);
        let mut paths = vec![(Role::Primary, input.clone())];
        for buffer in gltf.buffers() {
            if let gltf::buffer::Source::Uri(uri) = buffer.source()
                && !uri.starts_with("data:")
            {
                paths.push((
                    Role::Buffer(buffer.index()),
                    root.join(safe_external_buffer_path(uri)?),
                ));
            }
        }
        for image in gltf.images() {
            if let gltf::image::Source::Uri { uri, .. } = image.source()
                && !uri.starts_with("data:")
                && let Ok(key) = safe_external_buffer_path(uri)
            {
                // Repair does not decode images, but must preserve their
                // declared local files and even their currently missing keys.
                paths.push((Role::Image(image.index()), root.join(key)));
            }
        }
        let sources = paths
            .into_iter()
            .map(|(role, path)| {
                let captured = match file_id(&path) {
                    Ok(id) => Ok(Some(id)),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(error),
                };
                Source {
                    role,
                    path,
                    captured,
                }
            })
            .collect();
        Ok(Self {
            sources,
            #[cfg(unix)]
            primary_mode,
        })
    }

    pub(super) fn preflight(
        &self,
        output: &Path,
        gltf: &gltf::Gltf,
    ) -> Result<Destinations, FixError> {
        let mut entries = vec![Entry {
            role: Role::Primary,
            path: output.to_owned(),
            output: true,
        }];
        for buffer in gltf.buffers() {
            if let gltf::buffer::Source::Uri(uri) = buffer.source()
                && !uri.starts_with("data:")
            {
                entries.push(Entry {
                    role: Role::Buffer(buffer.index()),
                    path: parent(output).join(safe_external_buffer_path(uri)?),
                    output: true,
                });
            }
        }
        entries.extend(self.sources.iter().map(|source| Entry {
            role: source.role,
            path: source.path.clone(),
            output: false,
        }));
        let mut destinations = Destinations::default();
        match self.admit(&entries, &mut destinations) {
            Ok(()) => Ok(destinations),
            Err(error) => Err(destinations.refuse(error)),
        }
    }

    fn admit(&self, entries: &[Entry], destinations: &mut Destinations) -> Result<(), FixError> {
        let mut groups = BTreeMap::<FileId, Vec<&Entry>>::new();
        for (source, entry) in self
            .sources
            .iter()
            .zip(&entries[entries.len() - self.sources.len()..])
        {
            match &source.captured {
                Ok(Some(id)) => groups.entry(*id).or_default().push(entry),
                Ok(None) => {}
                Err(error) => return Err(inspect_error(&source.path, error).into()),
            }
        }

        // Missing names are reserved in the actual namespace. A private child
        // directory cannot reproduce sibling-dependent DOS short aliases or
        // per-directory case rules. CREATE_NEW never truncates an existing file.
        for entry in entries.iter().filter(|entry| entry.output) {
            if let Some(Identity::Missing(path)) =
                resolve(&entry.path, true, 0).map_err(|e| inspect_error(&entry.path, &e))?
            {
                let mut options = destination_options(true);
                #[cfg(unix)]
                let primary_mode = if entry.role == Role::Primary {
                    use std::os::unix::fs::OpenOptionsExt;
                    let mode = *self.primary_mode.as_ref().map_err(|error| {
                        WriteError::RepairPublication(format!(
                            "cannot read captured source permissions: {error}"
                        ))
                    })?;
                    // Creation must never expose a private source under broader
                    // default permissions, even before admission finishes.
                    options.mode(mode);
                    Some(mode)
                } else {
                    None
                };
                match options.create_new(true).open(&path) {
                    Ok(file) => {
                        let id = match handle_id(&file) {
                            Ok(id) => id,
                            Err(error) => {
                                let mut detail = format!(
                                    "cannot identify new reservation {}: {error}",
                                    path.display()
                                );
                                if let Err(cleanup) = remove_reservation(&file, &path) {
                                    detail.push_str(&format!(
                                        "; could not remove reservation: {cleanup}"
                                    ));
                                }
                                return Err(WriteError::RepairPublication(detail).into());
                            }
                        };
                        destinations.files.push(Destination {
                            role: entry.role,
                            path,
                            file,
                            id,
                            reserved: true,
                        });
                        #[cfg(unix)]
                        if let Some(mode) = primary_mode {
                            use std::os::unix::fs::PermissionsExt;
                            // Restore source bits removed by umask, through the
                            // owned handle. Existing destinations are untouched.
                            destinations
                                .files
                                .last()
                                .expect("just retained the reservation")
                                .file
                                .set_permissions(fs::Permissions::from_mode(mode))
                                .map_err(|error| inspect_error(&entry.path, &error))?;
                        }
                    }
                    // A prior reservation may have created this alias. Resolve
                    // every entry again below, after ALL prospective names exist.
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(inspect_error(&path, &e).into()),
                }
            }
        }

        let mut output_ids = Vec::new();
        for entry in entries {
            match resolve(&entry.path, entry.output, 0)
                .map_err(|e| inspect_error(&entry.path, &e))?
            {
                Some(Identity::Existing(id)) => {
                    groups.entry(id).or_default().push(entry);
                    if entry.output {
                        output_ids.push((entry, id));
                    }
                }
                _ if entry.output => {
                    return Err(WriteError::RepairPublication(format!(
                        "destination {} disappeared during preflight",
                        entry.path.display()
                    ))
                    .into());
                }
                _ => {} // A still-missing source is not occupied by an output.
            }
        }
        let in_place = groups.values().any(|group| {
            group
                .iter()
                .any(|entry| entry.output && entry.role == Role::Primary)
                && group
                    .iter()
                    .any(|entry| !entry.output && entry.role == Role::Primary)
        });
        for group in groups.values() {
            let mut outputs = group.iter().filter(|entry| entry.output);
            let Some(first) = outputs.next() else {
                continue;
            };
            if let Some(other) = outputs.next() {
                return Err(WriteError::RepairPublication(format!(
                    "output {} {} aliases output {} {}; choose distinct primary and buffer files",
                    first.role,
                    first.path.display(),
                    other.role,
                    other.path.display()
                ))
                .into());
            }
            for source in group.iter().filter(|entry| !entry.output) {
                if !(in_place && first.role == source.role) {
                    return Err(WriteError::RepairPublication(format!(
                        "output {} {} aliases source {} {}; choose a separate output directory or repair the original in place",
                        first.role, first.path.display(), source.role, source.path.display()
                    )).into());
                }
            }
        }
        let reserved_ids: BTreeSet<_> = destinations.files.iter().map(|file| file.id).collect();
        for (entry, id) in output_ids {
            if reserved_ids.contains(&id) {
                continue;
            }
            let file = destination_options(false)
                .open(&entry.path)
                .map_err(|e| inspect_error(&entry.path, &e))?;
            if handle_id(&file).map_err(|e| inspect_error(&entry.path, &e))? != id {
                return Err(WriteError::RepairPublication(format!(
                    "destination {} changed during preflight",
                    entry.path.display()
                ))
                .into());
            }
            destinations.files.push(Destination {
                role: entry.role,
                path: entry.path.clone(),
                file,
                id,
                reserved: false,
            });
        }
        Ok(())
    }
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn inspect_error(path: &Path, error: &io::Error) -> WriteError {
    WriteError::RepairPublication(format!(
        "cannot prepare repair publication for {}: {error}",
        path.display()
    ))
}

/// Resolve what `fs::write` would follow, not the entry a rename replaces.
fn resolve(path: &Path, output: bool, links: usize) -> io::Result<Option<Identity>> {
    match fs::metadata(path) {
        Ok(metadata) => {
            if output && !metadata.is_file() {
                return Err(io::Error::other("output is not a regular file"));
            }
            return file_id(path).map(|id| Some(Identity::Existing(id)));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if links >= 64 {
                return Err(io::Error::other("cannot resolve dangling symlink chain"));
            }
            let target = fs::read_link(path)?;
            return resolve(&parent(path).join(target), output, links + 1);
        }
        Ok(_) => return Err(io::Error::other("cannot resolve followed file")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    match fs::metadata(parent(path)) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Err(io::Error::other("destination parent is not a directory")),
        Err(error) if !output && error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    if path.file_name().is_none() {
        return Err(io::Error::other("path has no file name"));
    }
    // Preserve the path's actual Win32/DOS spelling. Joining a canonicalized
    // verbatim parent would change trailing-dot and other namespace semantics.
    Ok(Some(Identity::Missing(path.to_owned())))
}

fn destination_options(reservation: bool) -> fs::OpenOptions {
    let mut options = fs::OpenOptions::new();
    options.write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            DELETE, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
        if reservation {
            // DELETE lets cleanup remove exactly the owned file by handle.
            // With no delete sharing, its name cannot be replaced while held.
            options.access_mode(FILE_GENERIC_WRITE | DELETE);
        }
    }
    #[cfg(not(windows))]
    let _ = reservation;
    options
}

#[cfg(unix)]
fn remove_reservation(file: &fs::File, path: &Path) -> io::Result<()> {
    let id = handle_id(file)?;
    match file_id(path) {
        Ok(current) if current == id => fs::remove_file(path),
        Ok(_) => Ok(()), // An unrelated replacement is never ours to remove.
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(windows)]
fn remove_reservation(file: &fs::File, _path: &Path) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
    };
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: this owned handle requested DELETE access, and the exact input
    // structure is live for the synchronous call. Close removes this file only.
    let result = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&raw const info).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn remove_reservation(_file: &fs::File, _path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "file identity unsupported",
    ))
}

#[cfg(unix)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FileId {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
fn file_id(path: &Path) -> io::Result<FileId> {
    metadata_id(&fs::metadata(path)?)
}

#[cfg(unix)]
fn handle_id(file: &fs::File) -> io::Result<FileId> {
    metadata_id(&file.metadata()?)
}

#[cfg(unix)]
fn metadata_id(metadata: &fs::Metadata) -> io::Result<FileId> {
    use std::os::unix::fs::MetadataExt;
    Ok(FileId {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(windows)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FileId {
    volume: u64,
    index: [u8; 16],
}

#[cfg(windows)]
fn metadata_handle(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    // Zero desired access can query identity without reading protected image
    // contents. Do not use OPEN_REPARSE_POINT: repair follows final symlinks.
    fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(windows)]
fn file_id(path: &Path) -> io::Result<FileId> {
    handle_id(&metadata_handle(path)?)
}

#[cfg(windows)]
fn handle_id(file: &fs::File) -> io::Result<FileId> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx,
    };
    let mut info = FILE_ID_INFO::default();
    // SAFETY: the live owned handle and exact writable output type remain
    // valid for the complete synchronous metadata call.
    let result = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&raw mut info).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(FileId {
        volume: info.VolumeSerialNumber,
        index: info.FileId.Identifier,
    })
}

#[cfg(not(any(unix, windows)))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FileId;

#[cfg(not(any(unix, windows)))]
fn file_id(_path: &Path) -> io::Result<FileId> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "repair requires filesystem file identity support",
    ))
}

#[cfg(not(any(unix, windows)))]
fn handle_id(_file: &fs::File) -> io::Result<FileId> {
    file_id(Path::new(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reserve(dir: &Path) -> Destinations {
        let input = dir.join("source.gltf");
        let bytes = br#"{"asset":{"version":"2.0"}}"#;
        fs::write(&input, bytes).unwrap();
        let gltf = gltf::Gltf::from_slice(bytes).unwrap();
        Origin::capture(&input, &gltf)
            .unwrap()
            .preflight(&dir.join("output.gltf"), &gltf)
            .unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn private_primary_is_private_while_reserved() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.gltf");
        let bytes = br#"{"asset":{"version":"2.0"}}"#;
        fs::write(&input, bytes).unwrap();
        fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
        let gltf = gltf::Gltf::from_slice(bytes).unwrap();
        let output = dir.path().join("output.gltf");
        let destinations = Origin::capture(&input, &gltf)
            .unwrap()
            .preflight(&output, &gltf)
            .unwrap();
        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read(&output).unwrap(), b"");
        drop(destinations);
        assert!(!output.exists());
    }

    #[cfg(unix)]
    #[test]
    fn replacement_is_neither_truncated_during_publication_nor_removed_on_refusal() {
        for publish in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut destinations = reserve(dir.path());
            let output = dir.path().join("output.gltf");
            let moved = dir.path().join("moved-reservation");
            fs::rename(&output, &moved).unwrap();
            fs::write(&output, b"unrelated replacement").unwrap();
            if publish {
                assert!(destinations.write_primary(b"repaired output").is_err());
            }
            drop(destinations);
            assert_eq!(fs::read(&output).unwrap(), b"unrelated replacement");
            assert_eq!(fs::read(&moved).unwrap(), b"");
        }
    }

    #[cfg(windows)]
    #[test]
    fn reservation_name_is_held_until_refusal_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let destinations = reserve(dir.path());
        let output = dir.path().join("output.gltf");
        assert!(fs::rename(&output, dir.path().join("moved")).is_err());
        assert!(fs::remove_file(&output).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"");
        drop(destinations);
        assert!(!output.exists());
        assert!(!dir.path().join("moved").exists());
    }
}
