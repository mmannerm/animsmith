//! Output streams a spawned CLI cannot deliver to.
//!
//! Several CLI contract tests run `animsmith` with a stdout whose reader is
//! gone, so its first write fails the way `animsmith lint … | head` does, and
//! then assert that the failure is diagnosed on stderr without rewriting the
//! command's own exit code.
//!
//! Building that pipe in the test process is not reliable while other tests
//! in the same binary are spawning children. In Rust 1.97's
//! `library/std/src/sys/pipe/unix.rs`, `std::io::pipe` creates the pair
//! atomically close-on-exec only where `pipe2` exists; on Apple targets — one
//! of the three this project's CI covers — it calls `pipe(2)` and *then* sets
//! `FD_CLOEXEC` on each end, and that toolchain's Apple spawn path
//! (`posix_spawn` without `POSIX_SPAWN_CLOEXEC_DEFAULT`) hands a child every
//! descriptor that is not yet marked. A child another test thread spawns
//! inside that window therefore inherits the read end and holds the pipe open
//! for its own lifetime; the CLI under test writes successfully into the pipe
//! buffer, no diagnosis is printed, and the assertion counting it fails. That
//! is the macOS-only flake in issue #690.
//!
//! [`ClosedStream`] removes the window rather than narrowing it: on Unix the
//! pipe is created between `fork` and `exec` in the child itself, so neither
//! end ever exists in a process that can pass it on. Windows keeps the plain
//! `std::io::pipe` form, where `CreatePipe` returns non-inheritable handles
//! and `CreateProcess` passes on only the handles a spawn marks inheritable.
//!
//! Closing the descriptor outright is not an alternative: Rust's runtime
//! reopens a missing standard descriptor as `/dev/null` at startup, and a
//! write to `/dev/null` succeeds. The write end has to stay open with nothing
//! reading it.

use std::process::{Command, Stdio};

/// Give a [`Command`] an output stream whose read end exists nowhere, so the
/// child's first write to it fails with a broken pipe.
///
/// No process other than the child can hold that read end on any of the three
/// platforms this project's CI covers; see the module documentation for how
/// each one gets there.
pub trait ClosedStream {
    /// Give the child a stdout that nothing is reading.
    fn closed_stdout(&mut self) -> &mut Self;

    /// Give the child a stderr that nothing is reading.
    fn closed_stderr(&mut self) -> &mut Self;
}

#[cfg(unix)]
impl ClosedStream for Command {
    fn closed_stdout(&mut self) -> &mut Self {
        self.stdout(Stdio::null());
        reader_less_pipe_on(self, libc::STDOUT_FILENO)
    }

    fn closed_stderr(&mut self) -> &mut Self {
        self.stderr(Stdio::null());
        reader_less_pipe_on(self, libc::STDERR_FILENO)
    }
}

/// Replace `target` in the spawned child with the write end of a pipe created
/// in that child, after `fork` and before `exec`.
#[cfg(unix)]
fn reader_less_pipe_on(command: &mut Command, target: libc::c_int) -> &mut Command {
    use std::os::unix::process::CommandExt;

    // SAFETY: the closure runs in the forked child, which is single-threaded
    // there, and before `exec`. It allocates nothing and calls only `pipe`,
    // `dup2` and `close`, which are async-signal-safe, so it is safe to run
    // after a fork from this multi-threaded test binary.
    unsafe {
        command.pre_exec(move || {
            let mut ends: [libc::c_int; 2] = [-1; 2];
            if libc::pipe(ends.as_mut_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            let [reader, writer] = ends;
            // The `Stdio::null` above leaves `target` open when this runs, and
            // `pipe` returns the two lowest free descriptors, so neither end
            // it just handed back can be `target` itself.
            loop {
                if libc::dup2(writer, target) >= 0 {
                    break;
                }
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::EINTR) {
                    libc::close(reader);
                    libc::close(writer);
                    return Err(error);
                }
            }
            // `target` is now the only descriptor for the write end, and the
            // read end leaves with these closes.
            libc::close(reader);
            libc::close(writer);
            Ok(())
        });
    }
    command
}

#[cfg(windows)]
impl ClosedStream for Command {
    fn closed_stdout(&mut self) -> &mut Self {
        self.stdout(reader_less_pipe())
    }

    fn closed_stderr(&mut self) -> &mut Self {
        self.stderr(reader_less_pipe())
    }
}

/// The write end of a pipe whose read end is already dropped.
#[cfg(windows)]
fn reader_less_pipe() -> Stdio {
    let (reader, writer) = std::io::pipe().expect("creates a pipe");
    drop(reader);
    Stdio::from(writer)
}
